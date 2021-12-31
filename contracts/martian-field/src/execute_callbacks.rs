use std::cmp;
use std::cmp::Ordering;

use cosmwasm_std::{
    attr, Addr, Attribute, Decimal, DepsMut, Env, Event, Response, StdError, StdResult, Uint128,
};

use cw_asset::{Asset, AssetInfo, AssetList};

use fields_of_mars::martian_field::msg::CallbackMsg;
use fields_of_mars::martian_field::{Position, Snapshot, State};

use crate::health::compute_health;
use crate::state::{CACHED_USER_ADDR, CONFIG, POSITION, SNAPSHOT, STATE};

static DEFAULT_BOND_UNITS_PER_SHARE_BONDED: Uint128 = Uint128::new(1_000_000);
static DEFAULT_DEBT_UNITS_PER_ASSET_BORROWED: Uint128 = Uint128::new(1_000_000);

pub fn provide_liquidity(
    deps: DepsMut,
    user_addr_option: Option<Addr>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // if `user_addr` is provided, we load the user's position and provide the user's unlocked assets
    // if not provided, we load the state and provide the state's pending rewards
    let mut state = State::default();
    let mut position = Position::default();
    let assets: &mut AssetList;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    // we provide *all* available primary and secondary assets, assuming they are close in value.
    // it is strongly recommended to use `slippage_tolerance` parameter here
    let mut primary_asset_to_provide = assets
        .find(&config.primary_asset_info)
        .cloned()
        .ok_or_else(|| StdError::generic_err("no primary asset available"))?;
    let mut secondary_asset_to_provide = assets
        .find(&config.secondary_asset_info)
        .cloned()
        .ok_or_else(|| StdError::generic_err("no secondary asset available"))?;

    // all assets to be transferred must have tax deducted!
    primary_asset_to_provide.deduct_tax(&deps.querier)?;
    secondary_asset_to_provide.deduct_tax(&deps.querier)?;

    // the total cost for providing liquidity is the amount to be provided plus tax. we deduct these
    // amounts from the available assets
    let mut primary_asset_to_deduct = primary_asset_to_provide.clone();
    primary_asset_to_deduct.add_tax(&deps.querier)?;
    let mut secondary_asset_to_deduct = secondary_asset_to_provide.clone();
    secondary_asset_to_deduct.add_tax(&deps.querier)?;

    assets.deduct(&primary_asset_to_deduct)?;
    assets.deduct(&secondary_asset_to_deduct)?;

    // update storage
    // if `user_addr` is provided, we cache it so that it can be accessed when handling the reply
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
        CACHED_USER_ADDR.save(deps.storage, user_addr)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    Ok(Response::new()
        .add_submessages(config.primary_pair.provide_submsgs(
            0,
            &[primary_asset_to_provide.clone(), secondary_asset_to_provide.clone()],
            slippage_tolerance,
        )?)
        .add_attribute("action", "martian_field :: callback :: provide_liquidity")
        .add_attribute("primary_provided", primary_asset_to_provide.amount)
        .add_attribute("primary_deducted", primary_asset_to_deduct.amount)
        .add_attribute("secondary_provided", secondary_asset_to_provide.amount)
        .add_attribute("secondary_deducted", secondary_asset_to_deduct.amount))
}

pub fn withdraw_liquidity(deps: DepsMut, user_addr: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // We burn *all* of the user's unlocked liquidity tokens
    let liquidity_token_info = AssetInfo::cw20(config.primary_pair.liquidity_token.clone());
    let liquidity_token_to_burn = position
        .unlocked_assets
        .find(&liquidity_token_info)
        .cloned()
        .ok_or_else(|| StdError::generic_err("no unlocked share token available"))?;

    position.unlocked_assets.deduct(&liquidity_token_to_burn)?;
    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.save(deps.storage, &user_addr)?;

    Ok(Response::new()
        .add_submessage(config.primary_pair.withdraw_submsg(1, liquidity_token_to_burn.amount)?)
        .add_attribute("action", "martian_field :: callback :: withdraw_liquidity")
        .add_attribute("liquidity_token_burned", liquidity_token_to_burn.amount))
}

pub fn bond(deps: DepsMut, env: Env, user_addr_option: Option<Addr>) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // if a user address is provided, we bond the user's unlocked liquidity tokens
    // if not, we bond the state's pending liquidity tokens
    let mut position = Position::default();
    let assets: &mut AssetList;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        assets = &mut state.pending_rewards;
    }

    // we bond *all* of the available liquidity tokens
    let liquidity_token_info = AssetInfo::cw20(config.primary_pair.liquidity_token.clone());
    let liquidity_tokens_to_bond = assets
        .find(&liquidity_token_info)
        .cloned()
        .ok_or_else(|| StdError::generic_err("no liquidity token available"))?;

    // query how many liquidity tokens is currently being bonded by us
    let total_bonded_amount = config.astro_generator.query_bonded_amount(
        &deps.querier,
        &env.contract.address,
        &config.primary_pair.liquidity_token,
    )?;

    // calculate how by many the user's bond units should be increased
    // 1. if no user address is provided (meaning this is a harvest operation) then we don't
    // increment bond units
    // 2. if total bonded shares is zero, then we use the default value, which is defined as:
    // 1 unit of liquidity token bonded = 1,000,000 bond units
    let bond_units_to_add = if user_addr_option.is_none() {
        Uint128::zero()
    } else if total_bonded_amount.is_zero() {
        liquidity_tokens_to_bond.amount.checked_mul(DEFAULT_BOND_UNITS_PER_SHARE_BONDED)?
    } else {
        state.total_bond_units.multiply_ratio(liquidity_tokens_to_bond.amount, total_bonded_amount)
    };

    // Astro generator automatically withdraws pending rewards when bonding liquidity tokens
    // we query how much claimable rewards are there (assume exactly the same amount will be
    // withdrawn!) and increment the state's reinvestable rewards
    let rewards = config.astro_generator.query_rewards(
        &deps.querier,
        &env.contract.address,
        &config.primary_pair.liquidity_token,
    )?;

    assets.deduct(&liquidity_tokens_to_bond)?;
    state.pending_rewards.add_many(&rewards)?;
    state.total_bond_units = state.total_bond_units.checked_add(bond_units_to_add)?;
    STATE.save(deps.storage, &state)?;

    if let Some(user_addr) = &user_addr_option {
        position.bond_units = position.bond_units.checked_add(bond_units_to_add)?;
        POSITION.save(deps.storage, user_addr, &position)?;
    }

    Ok(Response::new()
        .add_message(
            config
                .astro_generator
                .bond_msg(&config.primary_pair.liquidity_token, liquidity_tokens_to_bond.amount)?,
        )
        .add_attribute("action", "martian_field :: callback :: bond")
        .add_attribute("bond_units_added", bond_units_to_add)
        .add_attribute("liquidity_token_bonded", liquidity_tokens_to_bond.amount))
}

pub fn unbond(
    deps: DepsMut,
    env: Env,
    user_addr: Addr,
    bond_units_to_deduct: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // Query how many share tokens is currently being bonded by us
    let total_bonded_amount = config.astro_generator.query_bonded_amount(
        &deps.querier,
        &env.contract.address,
        &config.primary_pair.liquidity_token,
    )?;

    // Calculate how many share tokens to unbond according the `bond_units_to_deduct`
    let amount_to_unbond =
        total_bonded_amount.multiply_ratio(bond_units_to_deduct, state.total_bond_units);
    let liquidity_token_to_unbond =
        Asset::cw20(config.primary_pair.liquidity_token.clone(), amount_to_unbond);

    // Astro generator automatically withdraws pending rewards when unbonding liquidity tokens
    // we query how much claimable rewards are there (assume exactly the same amount will be
    // withdrawn!) and increment the state's reinvestable rewards
    let rewards = config.astro_generator.query_rewards(
        &deps.querier,
        &env.contract.address,
        &config.primary_pair.liquidity_token,
    )?;

    state.total_bond_units = state.total_bond_units.checked_sub(bond_units_to_deduct)?;
    state.pending_rewards.add_many(&rewards)?;
    position.bond_units = position.bond_units.checked_sub(bond_units_to_deduct)?;
    position.unlocked_assets.deduct(&liquidity_token_to_unbond)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(
            config
                .astro_generator
                .unbond_msg(&config.primary_pair.liquidity_token, amount_to_unbond)?,
        )
        .add_attribute("action", "martian_field :: callback :: unbond")
        .add_attribute("bond_units_deducted", bond_units_to_deduct)
        .add_attribute("liquidity_token_unbonded", amount_to_unbond))
}

pub fn borrow(
    deps: DepsMut,
    env: Env,
    user_addr: Addr,
    borrow_amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // calculate how by many the user's debt units should be increased
    // if total debt is zero, then we define 1 unit of asset borrowed = 1,000,000 debt unit
    let total_debt_amount = config.red_bank.query_user_debt(
        &deps.querier,
        &env.contract.address,
        &config.secondary_asset_info,
    )?;
    let debt_units_to_add = if total_debt_amount.is_zero() {
        borrow_amount * DEFAULT_DEBT_UNITS_PER_ASSET_BORROWED
    } else {
        state.total_debt_units.multiply_ratio(borrow_amount, total_debt_amount)
    };

    // the actual amount we'll receive from Red Bank is the borrow amount minus tax. we increase the
    // user's unlocked secondary asset by this amount
    let secondary_asset_to_borrow = Asset::new(config.secondary_asset_info.clone(), borrow_amount);
    let mut secondary_asset_to_add = secondary_asset_to_borrow.clone();
    secondary_asset_to_add.deduct_tax(&deps.querier)?;

    state.total_debt_units += debt_units_to_add;
    position.debt_units += debt_units_to_add;
    position.unlocked_assets.add(&secondary_asset_to_add)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.borrow_msg(&secondary_asset_to_borrow)?)
        .add_attribute("action", "martian_field :: callback :: borrow")
        .add_attribute("debt_units_added", debt_units_to_add)
        .add_attribute("secondary_borrowed", secondary_asset_to_borrow.amount)
        .add_attribute("secondary_added", secondary_asset_to_add.amount))
}

pub fn repay(
    deps: DepsMut,
    env: Env,
    user_addr: Addr,
    repay_amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    let total_debt_amount = config.red_bank.query_user_debt(
        &deps.querier,
        &env.contract.address,
        &config.secondary_asset_info,
    )?;

    let debt_amount = total_debt_amount.multiply_ratio(position.debt_units, state.total_debt_units);

    // We only repay up to the debt amount
    let repay_amount = cmp::min(repay_amount, debt_amount);

    // Calculate how by many the user's debt units should be deducted
    let debt_units_to_deduct = if debt_amount.is_zero() {
        Uint128::zero()
    } else {
        position.debt_units.multiply_ratio(repay_amount, debt_amount)
    };

    // NOTE: `repay_amount` is the amount to be delivered to Red Bank. the total cost of making this
    // transfer is `repay_amount` plus tax. we deduct the total amount from the user's unlocked
    // secondary asset
    let secondary_asset_to_repay = Asset::new(config.secondary_asset_info.clone(), repay_amount);
    let mut secondary_asset_to_deduct = secondary_asset_to_repay.clone();
    secondary_asset_to_deduct.add_tax(&deps.querier)?;

    state.total_debt_units -= debt_units_to_deduct;
    position.debt_units -= debt_units_to_deduct;
    position.unlocked_assets.deduct(&secondary_asset_to_deduct)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.repay_msg(&secondary_asset_to_repay)?)
        .add_attribute("action", "martian_field :: callback :: repay")
        .add_attribute("debt_units_deducted", debt_units_to_deduct)
        .add_attribute("secondary_repaid", secondary_asset_to_repay.amount)
        .add_attribute("secondary_deducted", secondary_asset_to_deduct.amount))
}

pub fn swap(
    deps: DepsMut,
    user_addr_option: Option<Addr>,
    offer_asset_info: AssetInfo,
    offer_amount_option: Option<Uint128>,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // if `user_addr` is provided, we load the user's position and swap the user's unlocked assets
    // if not provided, we load the state and swap the state's pending rewards
    let mut state = State::default();
    let mut position = Position::default();
    let assets: &mut AssetList;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    // we only perform two kinds of swaps:
    // primary >> secondary; in this case, we use the primary-secondary pair
    // ASTRO >> secondary; in this case, we use the ASTRO-secondary pair
    let pair = if offer_asset_info == config.primary_asset_info {
        &config.primary_pair
    } else if offer_asset_info == config.astro_token_info {
        &config.astro_pair
    } else {
        return Err(StdError::generic_err(format!(
            "invalid offer asset: {}",
            offer_asset_info.to_string()
        )));
    };

    // if swap amount is unspecified, we swap all that's available
    let offer_asset = if let Some(offer_amount) = offer_amount_option {
        Asset::new(offer_asset_info, offer_amount)
    } else {
        assets
            .find(&offer_asset_info)
            .cloned()
            .unwrap_or_else(|| Asset::new(offer_asset_info, Uint128::zero()))
    };

    // if the deliverable amount after tax is zero, we do nothing
    let mut offer_asset_to_send = offer_asset.clone();
    offer_asset_to_send.deduct_tax(&deps.querier)?;
    if offer_asset_to_send.amount.is_zero() {
        return Ok(Response::default());
    }

    // deduct offer asset from the available amount
    let mut offer_asset_to_deduct = offer_asset_to_send.clone();
    offer_asset_to_deduct.deduct_tax(&deps.querier)?;
    assets.deduct(&offer_asset_to_deduct)?;

    // update storage
    // if `user_addr` is provided, we cache it so that it can be accessed when handling the reply
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
        CACHED_USER_ADDR.save(deps.storage, user_addr)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    Ok(Response::new()
        .add_submessage(pair.swap_submsg(2, &offer_asset, belief_price, max_spread)?)
        .add_attribute("action", "martian_field :: callback :: swap")
        .add_attribute("asset_offered", offer_asset.to_string())
        .add_attribute("asset_deducted", offer_asset_to_deduct.to_string()))
}

pub fn balance(
    deps: DepsMut,
    env: Env,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // find the available amounts of primary and secondary assets
    let primary_asset_amount = match state.pending_rewards.find(&config.primary_asset_info) {
        Some(asset) => asset.amount,
        None => Uint128::zero(),
    };
    let secondary_asset_amount = match state.pending_rewards.find(&config.secondary_asset_info) {
        Some(asset) => asset.amount,
        None => Uint128::zero(),
    };

    // query the prices of the two assets
    let primary_asset_price =
        config.oracle.query_price(&deps.querier, &config.primary_asset_info)?;
    let secondary_asset_price =
        config.oracle.query_price(&deps.querier, &config.secondary_asset_info)?;

    // calculate the values of available assets
    let primary_asset_value = primary_asset_amount * primary_asset_price;
    let secondary_asset_value = secondary_asset_amount * secondary_asset_price;

    // if primary_asset_value > secondary_asset_value, we swap primary >> secondary
    // if secondary_asset_value > primary_asset_value, we swap secondary >> primary
    // if equal, we skip
    let offer_asset_info = match primary_asset_value.cmp(&secondary_asset_value) {
        Ordering::Greater => config.primary_asset_info,
        Ordering::Less => config.secondary_asset_info,
        Ordering::Equal => return Ok(Response::default()),
    };

    // the amount to be swapped is the amount corresponding to half of the value difference
    //
    // e.g. we have $120 worth of UST and $100 worth of ANC. the diff in value is $20, so we swap
    // $20 / 2 = $10 worth of UST to ANC. ideally, this should leave us $110 worth of each
    //
    // in reality, considering slippage, commission, and tax, we will end up with $110 worth of UST
    // and **slight less than $110 worth** of ANC, so this method is not very optimized. the best
    // way is to solve a quadratic function which contains terms describing slippage and commission
    // rate to find the optimal swap amount. i have worked out the math somewhere else and will later
    // implement it as a separate smart contract
    //
    // for the time being, the less optimial method is ok as long as we harvest frequently - that is,
    // the amount that needs to be swapped is not very large at each harvest, so it should not incur
    // too much slippage
    let higher_value = cmp::max(primary_asset_value, secondary_asset_value);
    let lower_value = cmp::min(primary_asset_value, secondary_asset_value);
    let value_diff = higher_value - lower_value; // don't need underflow check here
    let value_to_swap = value_diff.multiply_ratio(1u128, 2u128);
    let amount_to_swap = value_to_swap.multiply_ratio(value_to_swap, higher_value);

    // if amount to swap is zero, we do nothing
    // if amount to swap is non-zero, we invoke the `Swap` callback
    let mut res = Response::new();
    if !amount_to_swap.is_zero() {
        res = res.add_message(
            CallbackMsg::Swap {
                user_addr: None,
                offer_asset_info: offer_asset_info.clone(),
                offer_amount: Some(amount_to_swap),
                belief_price,
                max_spread,
            }
            .into_cosmos_msg(&env.contract.address)?,
        );
    }

    Ok(res
        .add_attribute("action", "martian_field :: callback :: balance")
        .add_attribute("primary_asset_amount", primary_asset_amount)
        .add_attribute("secondary_asset_amount", secondary_asset_amount)
        .add_attribute("primary_asset_price", primary_asset_price.to_string())
        .add_attribute("secondary_asset_price", secondary_asset_price.to_string())
        .add_attribute("primary_asset_value", primary_asset_value)
        .add_attribute("secondary_asset_value", secondary_asset_value)
        .add_attribute("offer_asset", Asset::new(offer_asset_info, amount_to_swap).to_string()))
}

pub fn refund(
    deps: DepsMut,
    user_addr: Addr,
    recipient_addr: Addr,
    percentage: Decimal,
) -> StdResult<Response> {
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // 1. apply percentage
    // 2. deduct tax
    // 3. purge assets with zero amount
    let mut assets_to_refund = position.unlocked_assets.clone();
    assets_to_refund
        .apply(|asset| asset.amount = asset.amount * percentage)
        .deduct_tax(&deps.querier)?
        .purge();

    // the cost for refunding an asset is its amount plus tax. we deduct this amount from the user's
    // unlocked assets
    let mut assets_to_deduct = assets_to_refund.clone();
    assets_to_deduct.add_tax(&deps.querier)?;
    position.unlocked_assets.deduct_many(&assets_to_deduct)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    let refund_attrs: Vec<Attribute> = assets_to_refund
        .to_vec()
        .iter()
        .map(|asset| attr("asset_refunded", asset.to_string()))
        .collect();
    let deduct_attrs: Vec<Attribute> = assets_to_deduct
        .to_vec()
        .iter()
        .map(|asset| attr("asset_deducted", asset.to_string()))
        .collect();

    Ok(Response::new()
        .add_messages(assets_to_refund.transfer_msgs(&recipient_addr)?)
        .add_attribute("action", "martian_field :: callback :: refund")
        .add_attribute("recipient", recipient_addr.to_string())
        .add_attributes(refund_attrs)
        .add_attributes(deduct_attrs))
}

pub fn assert_health(deps: DepsMut, env: Env, user_addr: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    let health = compute_health(&deps.querier, &env, &config, &state, &position)?;

    // If ltv is Some(ltv), we assert it is no larger than `config.max_ltv`
    // If it is None, meaning `bond_value` is zero, we assert debt is also zero
    let healthy = if let Some(ltv) = health.ltv {
        ltv <= config.max_ltv
    } else {
        health.debt_value.is_zero()
    };

    // Convert `ltv` to String so that it can be recorded in logs
    let ltv_str = if let Some(ltv) = health.ltv {
        ltv.to_string()
    } else {
        "undefined".to_string()
    };

    if !healthy {
        return Err(StdError::generic_err(format!("ltv greater than threshold: {}", ltv_str)));
    }

    let event = Event::new("field_position_changed")
        .add_attribute("timestamp", env.block.time.seconds().to_string())
        .add_attribute("height", env.block.height.to_string())
        .add_attribute("user_addr", &user_addr)
        .add_attribute("bond_units", position.bond_units)
        .add_attribute("debt_units", position.debt_units)
        .add_attribute("bond_value", health.bond_value)
        .add_attribute("debt_value", health.debt_value)
        .add_attribute("ltv", &ltv_str);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: callback :: assert_health")
        .add_event(event))
}

pub fn snapshot(deps: DepsMut, env: Env, user_addr: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    let health = compute_health(&deps.querier, &env, &config, &state, &position)?;

    let snapshot = Snapshot {
        time: env.block.time.seconds(),
        height: env.block.height,
        position: position.into(),
        health,
    };

    SNAPSHOT.save(deps.storage, &user_addr, &snapshot)?;

    Ok(Response::new().add_attribute("action", "martian_field :: callback :: snapshot"))
}
