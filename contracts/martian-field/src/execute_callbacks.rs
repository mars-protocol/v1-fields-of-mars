use std::cmp;

use cosmwasm_std::{
    attr, Addr, Attribute, CosmosMsg, Decimal, DepsMut, Env, Event, Response, StdError, StdResult,
    Uint128,
};

use fields_of_mars::adapters::{Asset, AssetInfo};
use fields_of_mars::martian_field::{Position, Snapshot, State};

use crate::helpers::{
    add_asset_to_array, compute_health, deduct_asset_from_array, find_asset_in_array,
};
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
    let assets: &mut Vec<Asset>;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    // We provide *all* available primary and secondary assets, assuming they are close in value.
    // it is strongly recommended to use `slippage_tolerance` parameter here
    // NOTE: must deduct tax here!
    let primary_asset_to_provide = assets
        .iter()
        .cloned()
        .find(|asset| asset.info == config.primary_asset_info)
        .map(|asset| asset.deduct_tax(&deps.querier))
        .transpose()?
        .ok_or_else(|| StdError::generic_err("no primary asset available"))?;
    let secondary_asset_to_provide = assets
        .iter()
        .cloned()
        .find(|asset| asset.info == config.secondary_asset_info)
        .map(|asset| asset.deduct_tax(&deps.querier))
        .transpose()?
        .ok_or_else(|| StdError::generic_err("no secondary asset available"))?;

    // The total cost for providing liquidity is the amount to be provided plus tax. We deduct these
    // amounts from the user's unlocked assets
    let primary_asset_to_deduct = primary_asset_to_provide.add_tax(&deps.querier)?;
    let secondary_asset_to_deduct = secondary_asset_to_provide.add_tax(&deps.querier)?;

    deduct_asset_from_array(assets, &primary_asset_to_deduct)?;
    deduct_asset_from_array(assets, &secondary_asset_to_deduct)?;

    // update storage
    // if `user_addr` is provided, we cache it so that it can be accessed when handling the reply
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
        CACHED_USER_ADDR.save(deps.storage, user_addr)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    Ok(Response::new()
        .add_submessages(config.pair.provide_submsgs(
            0,
            &[primary_asset_to_provide.clone(), secondary_asset_to_provide.clone()],
            slippage_tolerance,
        )?)
        .add_attribute("action", "martian_field :: callback :: provide_liquidity")
        .add_attribute("primary_provided_amount", primary_asset_to_provide.amount)
        .add_attribute("primary_deducted_amount", primary_asset_to_deduct.amount)
        .add_attribute("secondary_provided_amount", secondary_asset_to_provide.amount)
        .add_attribute("secondary_deducted_amount", secondary_asset_to_deduct.amount))
}

pub fn withdraw_liquidity(deps: DepsMut, user_addr: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // We burn *all* of the user's unlocked share tokens
    let share_asset_to_burn = position
        .unlocked_assets
        .iter()
        .cloned()
        .find(|asset| asset.info == AssetInfo::Cw20(config.pair.liquidity_token.clone()))
        .ok_or_else(|| StdError::generic_err("no unlocked share token available"))?;

    deduct_asset_from_array(&mut position.unlocked_assets, &share_asset_to_burn)?;

    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.save(deps.storage, &user_addr)?;

    Ok(Response::new()
        .add_submessage(config.pair.withdraw_submsg(1, share_asset_to_burn.amount)?)
        .add_attribute("action", "martian_field :: callback :: withdraw_liquidity")
        .add_attribute("share_burned_amount", share_asset_to_burn.amount))
}

pub fn bond(deps: DepsMut, env: Env, user_addr_option: Option<Addr>) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // if a user address is provided, we bond the user's unlocked liquidity tokens
    // if not, we bond the state's pending liquidity tokens
    let mut position = Position::default();
    let assets: &mut Vec<Asset>;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        assets = &mut state.pending_rewards;
    }

    // We bond *all* of the available liquidity tokens
    let liquidity_tokens_to_bond = assets
        .iter()
        .cloned()
        .find(|asset| asset.info == AssetInfo::Cw20(config.pair.liquidity_token.clone()))
        .ok_or_else(|| StdError::generic_err("no liquidity token available"))?;

    // Query how many liquidity tokens is currently being bonded by us
    let (total_bonded_amount, _) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // Calculate how by many the user's bond units should be increased
    // 1. If no user address is provided (meaning this is a harvest operation) then we don't
    // increment bond units
    // 2. If total bonded shares is zero, then we define 1 unit of share token bonded = 1,000,000 bond units
    let bond_units_to_add = if user_addr_option.is_none() {
        Uint128::zero()
    } else if total_bonded_amount.is_zero() {
        liquidity_tokens_to_bond.amount * DEFAULT_BOND_UNITS_PER_SHARE_BONDED
    } else {
        state.total_bond_units.multiply_ratio(liquidity_tokens_to_bond.amount, total_bonded_amount)
    };

    // deduct available amount
    deduct_asset_from_array(assets, &liquidity_tokens_to_bond)?;

    // update state
    state.total_bond_units += bond_units_to_add;
    STATE.save(deps.storage, &state)?;

    // if a user address is provided, update the position
    if let Some(user_addr) = &user_addr_option {
        position.bond_units += bond_units_to_add;
        POSITION.save(deps.storage, &user_addr, &position)?;
    }

    Ok(Response::new()
        .add_message(config.staking.bond_msg(liquidity_tokens_to_bond.amount)?)
        .add_attribute("action", "martian_field :: callback :: bond")
        .add_attribute("bond_units_added", bond_units_to_add)
        .add_attribute("share_bonded_amount", liquidity_tokens_to_bond.amount))
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
    let (total_bonded_amount, _) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // Calculate how many share tokens to unbond according the `bond_units_to_deduct`
    let amount_to_unbond =
        total_bonded_amount.multiply_ratio(bond_units_to_deduct, state.total_bond_units);

    state.total_bond_units -= bond_units_to_deduct;
    position.bond_units -= bond_units_to_deduct;

    add_asset_to_array(
        &mut position.unlocked_assets,
        &Asset::cw20(&config.pair.liquidity_token, amount_to_unbond),
    );

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.staking.unbond_msg(amount_to_unbond)?)
        .add_attribute("action", "martian_field :: callback :: unbond")
        .add_attribute("bond_units_deducted", bond_units_to_deduct)
        .add_attribute("share_unbonded_amount", amount_to_unbond))
}

pub fn borrow(
    deps: DepsMut,
    env: Env,
    user_addr: Addr,
    borrow_amount: Uint128,
) -> StdResult<Response> {
    // If borrow amount is zero, we do nothing
    if borrow_amount.is_zero() {
        return Ok(Response::default());
    }

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    let secondary_asset_to_borrow = Asset::new(&config.secondary_asset_info, borrow_amount);

    let total_debt_amount = config.red_bank.query_user_debt(
        &deps.querier,
        &env.contract.address,
        &config.secondary_asset_info,
    )?;

    // Calculate how by many the user's debt units should be increased
    // If total debt is zero, then we define 1 unit of asset borrowed = 1,000,000 debt unit
    let debt_units_to_add = if total_debt_amount.is_zero() {
        secondary_asset_to_borrow.amount * DEFAULT_DEBT_UNITS_PER_ASSET_BORROWED
    } else {
        state.total_debt_units.multiply_ratio(secondary_asset_to_borrow.amount, total_debt_amount)
    };

    // This the actual amount we'll receive from Red Bank is the borrow amount minus tax. We increase
    // the user's unlocked secondary asset by this amount
    let secondary_asset_to_add = secondary_asset_to_borrow.deduct_tax(&deps.querier)?;

    state.total_debt_units += debt_units_to_add;
    position.debt_units += debt_units_to_add;

    add_asset_to_array(&mut position.unlocked_assets, &secondary_asset_to_add);

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.borrow_msg(&secondary_asset_to_borrow)?)
        .add_attribute("action", "martian_field :: callback :: borrow")
        .add_attribute("debt_units_added", debt_units_to_add)
        .add_attribute("secondary_borrowed_amount", borrow_amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}

pub fn repay(
    deps: DepsMut,
    env: Env,
    user_addr: Addr,
    repay_amount: Uint128,
) -> StdResult<Response> {
    // If repay amount is zero, we do nothing
    if repay_amount.is_zero() {
        return Ok(Response::default());
    }

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

    // NOTE: `repay_amount` is the amount to be delivered to Red Bank. The total cost of making this
    // transfer is `repay_amount` plus tax. We deduct this amount from the user's unlocked secondary asset
    let secondary_asset_to_repay = Asset::new(&config.secondary_asset_info, repay_amount);
    let secondary_asset_to_deduct = secondary_asset_to_repay.add_tax(&deps.querier)?;

    state.total_debt_units -= debt_units_to_deduct;
    position.debt_units -= debt_units_to_deduct;

    deduct_asset_from_array(&mut position.unlocked_assets, &secondary_asset_to_deduct)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.repay_msg(&secondary_asset_to_repay)?)
        .add_attribute("action", "martian_field :: callback :: repay")
        .add_attribute("debt_units_deducted", debt_units_to_deduct)
        .add_attribute("secondary_repaid_amount", secondary_asset_to_repay.amount)
        .add_attribute("secondary_deducted_amount", secondary_asset_to_deduct.amount))
}

pub fn swap(
    deps: DepsMut,
    user_addr_option: Option<Addr>,
    swap_amount_option: Option<Uint128>,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // if `user_addr` is provided, we load the user's position and swap the user's unlocked assets
    // if not provided, we load the state and swap the state's pending rewards
    let mut state = State::default();
    let mut position = Position::default();
    let assets: &mut Vec<Asset>;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    // if swap amount is unspecified, we swap all that's available, minux tax
    let offer_asset = if let Some(swap_amount) = swap_amount_option {
        Asset::new(&config.primary_asset_info, swap_amount)
    } else {
        find_asset_in_array(assets, &config.primary_asset_info).deduct_tax(&deps.querier)?
    };

    // if offer amount is zero, we do nothing
    if offer_asset.amount.is_zero() {
        return Ok(Response::default());
    }

    // deduct offer asset from the available amount
    let offer_asset_to_deduct = offer_asset.add_tax(&deps.querier)?;
    deduct_asset_from_array(assets, &offer_asset_to_deduct)?;

    // update storage
    // if `user_addr` is provided, we cache it so that it can be accessed when handling the reply
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
        CACHED_USER_ADDR.save(deps.storage, user_addr)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    Ok(Response::new()
        .add_submessage(config.pair.swap_submsg(2, &offer_asset, belief_price, max_spread)?)
        .add_attribute("action", "martian_field :: callback :: swap")
        .add_attribute("asset_offered", offer_asset.to_string())
        .add_attribute("asset_deducted", offer_asset_to_deduct.to_string()))
}

pub fn refund(
    deps: DepsMut,
    user_addr: Addr,
    recipient_addr: Addr,
    percentage: Decimal,
) -> StdResult<Response> {
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // NOTE:
    // 1. Must deduct tax
    // 2. Must filter off assets whose amount is zero
    let assets_to_refund: Vec<Asset> = position
        .unlocked_assets
        .iter()
        .map(|asset| asset * percentage)
        .map(|asset| asset.deduct_tax(&deps.querier).unwrap())
        .filter(|asset| !asset.amount.is_zero())
        .collect();

    // The cost for refunding an asset is the amount to refund plus tax. We deduct this amount from
    // the user's unlocked assets
    let assets_to_deduct: Vec<Asset> = assets_to_refund
        .iter()
        .map(|asset_to_refund| asset_to_refund.add_tax(&deps.querier).unwrap())
        .collect();

    for asset in &assets_to_deduct {
        deduct_asset_from_array(&mut position.unlocked_assets, asset)?;
    }

    POSITION.save(deps.storage, &user_addr, &position)?;

    let msgs: Vec<CosmosMsg> =
        assets_to_refund.iter().map(|asset| asset.transfer_msg(&recipient_addr).unwrap()).collect();

    let refund_attrs: Vec<Attribute> =
        assets_to_refund.iter().map(|asset| attr("asset_refunded", asset.to_string())).collect();
    let deduct_attrs: Vec<Attribute> =
        assets_to_deduct.iter().map(|asset| attr("asset_deducted", asset.to_string())).collect();

    Ok(Response::new()
        .add_messages(msgs)
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
