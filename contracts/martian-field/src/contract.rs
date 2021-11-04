use std::cmp;

use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Attribute, Binary, CosmosMsg, Decimal, Deps, DepsMut, Env,
    Event, MessageInfo, Reply, Response, StdError, StdResult, SubMsgExecutionResponse, Uint128,
};

use fields_of_mars::adapters::{Asset, AssetInfo, Pair};
use fields_of_mars::martian_field::msg::{CallbackMsg, ExecuteMsg, MigrateMsg, QueryMsg};
use fields_of_mars::martian_field::{
    Config, ConfigUnchecked, Health, PositionUnchecked, Snapshot, State,
};

use crate::helpers::*;
use crate::state::{CONFIG, POSITION, SNAPSHOT, STATE, TEMP_USER_ADDR};

static DEFAULT_BOND_UNITS_PER_SHARE_BONDED: Uint128 = Uint128::new(1_000_000);
static DEFAULT_DEBT_UNITS_PER_ASSET_BORROWED: Uint128 = Uint128::new(1_000_000);

//--------------------------------------------------------------------------------------------------
// Instantiate
//--------------------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ConfigUnchecked,
) -> StdResult<Response> {
    CONFIG.save(deps.storage, &msg.check(deps.api)?)?;
    STATE.save(deps.storage, &State::default())?;
    Ok(Response::default())
}

//--------------------------------------------------------------------------------------------------
// Execute
//--------------------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let api = deps.api;
    match msg {
        ExecuteMsg::UpdateConfig {
            new_config,
        } => execute_update_config(deps, env, info, new_config.check(api)?),
        ExecuteMsg::IncreasePosition {
            deposits,
        } => execute_increase_position(
            deps,
            env,
            info,
            deposits.iter().map(|deposit| deposit.check(api).unwrap()).collect(),
        ),
        ExecuteMsg::ReducePosition {
            bond_units,
            swap_amount,
            repay_amount,
        } => execute_reduce_position(deps, env, info, bond_units, swap_amount, repay_amount),
        ExecuteMsg::PayDebt {
            repay_amount,
        } => execute_pay_debt(deps, env, info, repay_amount),
        ExecuteMsg::Liquidate {
            user,
        } => execute_liquidate(deps, env, info, api.addr_validate(&user)?),
        ExecuteMsg::Harvest {} => execute_harvest(deps, env, info),
        ExecuteMsg::Callback(callback_msg) => execute_callback(deps, env, info, callback_msg),
    }
}

fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: Config,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.governance {
        return Err(StdError::generic_err("only governance can update config"));
    }

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::default())
}

fn execute_increase_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    deposits: Vec<Asset>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &info.sender).unwrap_or_default();

    // Find how much primary and secondary assets were received, respectively
    let primary_asset_deposited = deposits
        .iter()
        .find(|deposit| deposit.info == config.primary_asset_info)
        .cloned()
        .unwrap_or_else(|| Asset::new(&config.primary_asset_info, Uint128::zero()));
    let secondary_asset_deposited = deposits
        .iter()
        .find(|deposit| deposit.info == config.secondary_asset_info)
        .cloned()
        .unwrap_or_else(|| Asset::new(&config.secondary_asset_info, Uint128::zero()));

    // Increment the user's unlocked assets by the amount deposited
    let primary_asset_unlocked = add_unlocked_asset(&mut position, &primary_asset_deposited);
    let secondary_asset_unlocked = add_unlocked_asset(&mut position, &secondary_asset_deposited);
    POSITION.save(deps.storage, &info.sender, &position)?;

    // Calculate the amount of secondary asset to be borrowed
    //
    // We provide all unlocked primary assets to the liquidity pool. The amount of secondary asset
    // needed for liquidity provision is:
    // primary_asset_unlocked.amount * secondary_depth / primary_depth
    //
    // If the amount needed is bigger than the amount of secondary asset deposited, we borrow the
    // difference from Red Bank. Otherwise, we borrow zero
    let (primary_depth, secondary_depth, _) = config.pair.query_pool(
        &deps.querier,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;
    let secondary_borrow_amount = primary_asset_unlocked
        .amount
        .multiply_ratio(secondary_depth, primary_depth)
        .checked_sub(secondary_asset_unlocked.amount)
        .unwrap_or_else(|_| Uint128::zero());

    // For each deposit,
    // If it's a CW20, we transfer it from the user's wallet to us (must have allowance)
    // If it's a native token, we assert the amount was indeed transferred to us
    let mut messages: Vec<CosmosMsg> = vec![];

    for deposit in deposits.iter() {
        match &deposit.info {
            AssetInfo::Cw20 {
                ..
            } => {
                messages.push(deposit.transfer_from_msg(&info.sender, &env.contract.address)?);
            }
            AssetInfo::Native {
                ..
            } => {
                deposit.assert_sent_amount(&info.funds)?;
            }
        }
    }

    let callbacks = [
        CallbackMsg::Borrow {
            user_addr: info.sender.clone(),
            borrow_amount: secondary_borrow_amount,
        },
        CallbackMsg::ProvideLiquidity {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::Bond {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::AssertHealth {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::Snapshot {
            user_addr: info.sender,
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(Response::new()
        .add_messages(messages)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: execute :: increase_position")
        .add_attribute("primary_deposited_amount", primary_asset_deposited.amount)
        .add_attribute("secondary_deposited_amount", secondary_asset_deposited.amount))
}

fn execute_reduce_position(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bond_units: Uint128,
    swap_amount: Uint128,
    repay_amount: Uint128,
) -> StdResult<Response> {
    let callbacks = vec![
        CallbackMsg::Unbond {
            user_addr: info.sender.clone(),
            bond_units,
        },
        CallbackMsg::WithdrawLiquidity {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::Swap {
            user_addr: info.sender.clone(),
            swap_amount: Some(swap_amount),
        },
        CallbackMsg::Repay {
            user_addr: info.sender.clone(),
            repay_amount,
        },
        CallbackMsg::AssertHealth {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::Refund {
            user_addr: info.sender.clone(),
            recipient: info.sender.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::Snapshot {
            user_addr: info.sender,
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(Response::new()
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: execute :: reduce_position"))
}

fn execute_pay_debt(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    repay_amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &info.sender)?;

    // Find how much asset was deposited
    //
    // If secondary asset is a CW20, the deposit amount is exactly the amount to be repaid, and we
    // later transfer the amount from the user's wallet to us
    //
    // If secondary asset is a native token, we find how much was transferred with the message.
    // `deposit_amount` is the amount that's received by us
    // `repay_amount` is the amount to be repaid
    // Typically, `deposit_amount` needs to be slightly greater than `repay_amount` because repayment
    // requires tax. The unused amount will be refunded to the user
    let deposit_amount = if config.secondary_asset_info.is_cw20() {
        repay_amount
    } else {
        config.secondary_asset_info.find_sent_amount(&info.funds)
    };

    // Increment the user's unlocked secondary asset by the deposited amount
    let secondary_asset_deposited = Asset::new(&config.secondary_asset_info, deposit_amount);
    add_unlocked_asset(&mut position, &secondary_asset_deposited);
    POSITION.save(deps.storage, &info.sender, &position)?;

    // Construct messages. If secondary asset is a CW20, we transfer it from the user's wallet to us
    // (must be allowance). If it's a native token, we do nothing because we have already received it
    let msgs = if secondary_asset_deposited.info.is_cw20() {
        vec![secondary_asset_deposited.transfer_from_msg(&info.sender, &env.contract.address)?]
    } else {
        vec![]
    };

    let callbacks = [
        CallbackMsg::Repay {
            user_addr: info.sender.clone(),
            repay_amount,
        },
        // We don't really need to assert health here, but doing assert health emits a `position_changed`
        // event which is useful for logging
        CallbackMsg::AssertHealth {
            user_addr: info.sender.clone(),
        },
        // If the user paid more than what is owed, there will be some secondary asset remaining as
        // unlocked in the user's position. We do a refund to return it to the user
        CallbackMsg::Refund {
            user_addr: info.sender.clone(),
            recipient: info.sender.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::Snapshot {
            user_addr: info.sender,
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: execute :: pay_debt")
        .add_attribute("secondary_deposited_amount", secondary_asset_deposited.amount))
}

fn execute_liquidate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr)?;

    // The position must be active (LTV is not `None`) and the LTV must be greater than `max_ltv`
    let health = compute_health(&deps.querier, &env, &config, &state, &position)?;
    let ltv = health.ltv.ok_or_else(|| StdError::generic_err("position is closed"))?;

    if ltv <= config.max_ltv {
        return Err(StdError::generic_err("position is healthy"));
    }

    let callbacks = [
        CallbackMsg::Unbond {
            user_addr: user_addr.clone(),
            bond_units: position.bond_units,
        },
        CallbackMsg::WithdrawLiquidity {
            user_addr: user_addr.clone(),
        },
        CallbackMsg::Swap {
            user_addr: user_addr.clone(),
            swap_amount: None,
        },
        CallbackMsg::Repay {
            user_addr: user_addr.clone(),
            repay_amount: health.debt_value,
        },
        CallbackMsg::Refund {
            user_addr: user_addr.clone(),
            recipient: info.sender.clone(),
            percentage: config.bonus_rate,
        },
        CallbackMsg::Refund {
            user_addr: user_addr.clone(),
            recipient: user_addr.clone(),
            percentage: Decimal::one(),
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    let event = Event::new("liquidated")
        .add_attribute("liquidator_addr", info.sender)
        .add_attribute("user_addr", user_addr)
        .add_attribute("bond_units", position.bond_units)
        .add_attribute("debt_units", position.debt_units)
        .add_attribute("bond_value", health.bond_value)
        .add_attribute("debt_value", health.debt_value)
        .add_attribute("ltv", ltv.to_string());

    Ok(Response::new()
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: excute :: liquidate")
        .add_event(event))
}

fn execute_harvest(deps: DepsMut, env: Env, _info: MessageInfo) -> StdResult<Response> {
    // We use a fake address to track assets handled during harvesting
    let reward_addr = Addr::unchecked("reward");

    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &reward_addr).unwrap_or_default();

    // Find how much reward is available to be claimed
    let (_, reward_amount) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // If there is no reward to claim, then we do nothing
    if reward_amount.is_zero() {
        return Ok(Response::default());
    }

    // We assume the reward is in the primary asset
    // Among the claimable reward, a portion corresponding to `config.fee_rate` is charged as fee
    // and sent to the treasury address. Among the rest, half is to be swapped for the secondary asset
    let fee_amount = reward_amount * config.fee_rate;
    let reward_amount_after_fee = reward_amount - fee_amount;
    let swap_amount = reward_amount_after_fee.multiply_ratio(1u128, 2u128);

    let primary_asset_to_transfer = Asset::new(&config.primary_asset_info, fee_amount);
    let primary_asset_to_add = Asset::new(&config.primary_asset_info, reward_amount_after_fee);

    add_unlocked_asset(&mut position, &primary_asset_to_add);
    POSITION.save(deps.storage, &reward_addr, &position)?;

    let msgs = vec![
        config.staking.withdraw_msg()?,
        primary_asset_to_transfer.transfer_msg(&config.treasury)?,
    ];

    let callbacks = [
        CallbackMsg::Swap {
            user_addr: reward_addr.clone(),
            swap_amount: Some(swap_amount),
        },
        CallbackMsg::ProvideLiquidity {
            user_addr: reward_addr.clone(),
        },
        CallbackMsg::Bond {
            user_addr: reward_addr,
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    let event = Event::new("harvested")
        .add_attribute("timestamp", env.block.time.seconds().to_string())
        .add_attribute("height", env.block.height.to_string())
        .add_attribute("fee_amount", fee_amount)
        .add_attribute("reward_amount_after_fee", reward_amount_after_fee);

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: execute :: harvest")
        .add_event(event))
}

//--------------------------------------------------------------------------------------------------
// Callbacks
//--------------------------------------------------------------------------------------------------

fn execute_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> StdResult<Response> {
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }

    match msg {
        CallbackMsg::ProvideLiquidity {
            user_addr,
        } => callback_provide_liquidity(deps, env, info, user_addr),
        CallbackMsg::WithdrawLiquidity {
            user_addr,
        } => callback_withdraw_liquidity(deps, env, info, user_addr),
        CallbackMsg::Bond {
            user_addr,
        } => callback_bond(deps, env, info, user_addr),
        CallbackMsg::Unbond {
            user_addr,
            bond_units,
        } => callback_unbond(deps, env, info, user_addr, bond_units),
        CallbackMsg::Borrow {
            user_addr,
            borrow_amount,
        } => callback_borrow(deps, env, info, user_addr, borrow_amount),
        CallbackMsg::Repay {
            user_addr,
            repay_amount,
        } => callback_repay(deps, env, info, user_addr, repay_amount),
        CallbackMsg::Refund {
            user_addr,
            recipient,
            percentage,
        } => callback_refund(deps, env, info, user_addr, recipient, percentage),
        CallbackMsg::Swap {
            user_addr,
            swap_amount,
        } => callback_swap(deps, env, info, user_addr, swap_amount),
        CallbackMsg::AssertHealth {
            user_addr,
        } => callback_assert_health(deps, env, info, user_addr),
        CallbackMsg::Snapshot {
            user_addr,
        } => callback_snapshot(deps, env, info, user_addr),
    }
}

fn callback_provide_liquidity(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // We deposit *all* unlocked primary and secondary assets to AMM, assuming the functions invoking
    // this callback have already did borrowing or swaps such that the values of the assets are about
    // the same
    // NOTE: must deduct tax here!
    let primary_asset_to_provide = position
        .unlocked_assets
        .iter()
        .cloned()
        .find(|asset| asset.info == config.primary_asset_info)
        .map(|asset| asset.deduct_tax(&deps.querier).unwrap())
        .ok_or_else(|| StdError::generic_err("no unlocked primary asset available"))?;
    let secondary_asset_to_provide = position
        .unlocked_assets
        .iter()
        .cloned()
        .find(|asset| asset.info == config.secondary_asset_info)
        .map(|asset| asset.deduct_tax(&deps.querier).unwrap())
        .ok_or_else(|| StdError::generic_err("no unlocked secondary asset available"))?;

    // The total cost for providing liquidity is the amount to be provided plus tax. We deduct these
    // amounts from the user's unlocked assets
    let primary_asset_to_deduct = primary_asset_to_provide.add_tax(&deps.querier)?;
    let secondary_asset_to_deduct = secondary_asset_to_provide.add_tax(&deps.querier)?;

    deduct_unlocked_asset(&mut position, &primary_asset_to_deduct)?;
    deduct_unlocked_asset(&mut position, &secondary_asset_to_deduct)?;

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.save(deps.storage, &user_addr)?;

    Ok(Response::new()
        .add_submessages(config.pair.provide_submsgs(
            0,
            &[primary_asset_to_provide.clone(), secondary_asset_to_provide.clone()],
        )?)
        .add_attribute("action", "martian_field :: callback :: provide_liquidity")
        .add_attribute("primary_provided_amount", primary_asset_to_provide.amount)
        .add_attribute("primary_deducted_amount", primary_asset_to_deduct.amount)
        .add_attribute("secondary_provided_amount", secondary_asset_to_provide.amount)
        .add_attribute("secondary_deducted_amount", secondary_asset_to_deduct.amount))
}

fn callback_withdraw_liquidity(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // We burn *all* of the user's unlocked share tokens
    let share_asset_to_burn = position
        .unlocked_assets
        .iter()
        .cloned()
        .find(|asset| asset.info == AssetInfo::cw20(&config.pair.liquidity_token))
        .ok_or_else(|| StdError::generic_err("no unlocked share token available"))?;

    deduct_unlocked_asset(&mut position, &share_asset_to_burn)?;

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.save(deps.storage, &user_addr)?;

    Ok(Response::new()
        .add_submessage(config.pair.withdraw_submsg(1, share_asset_to_burn.amount)?)
        .add_attribute("action", "martian_field :: callback :: withdraw_liquidity")
        .add_attribute("share_burned_amount", share_asset_to_burn.amount))
}

fn callback_bond(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // We bond *all* of the user's unlocked share tokens
    let share_asset_to_bond = position
        .unlocked_assets
        .iter()
        .cloned()
        .find(|asset| asset.info == AssetInfo::cw20(&config.pair.liquidity_token))
        .ok_or_else(|| StdError::generic_err("no unlocked share token available"))?;

    // Query how many share tokens is currently being bonded by us
    let (total_bonded_amount, _) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // Calculate how by many the user's bond units should be increased
    // 1. If user address is the `reward` (meaning this is a harvest transaction) then we don't
    // increment bond units
    // 2. If total bonded shares is zero, then we define 1 unit of share token bonded = 1,000,000 bond units
    let bond_units_to_add = if user_addr == Addr::unchecked("reward") {
        Uint128::zero()
    } else if total_bonded_amount.is_zero() {
        share_asset_to_bond.amount * DEFAULT_BOND_UNITS_PER_SHARE_BONDED
    } else {
        state.total_bond_units.multiply_ratio(share_asset_to_bond.amount, total_bonded_amount)
    };

    state.total_bond_units += bond_units_to_add;
    position.bond_units += bond_units_to_add;

    deduct_unlocked_asset(&mut position, &share_asset_to_bond)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.staking.bond_msg(share_asset_to_bond.amount)?)
        .add_attribute("action", "martian_field :: callback :: bond")
        .add_attribute("bond_units_added", bond_units_to_add)
        .add_attribute("share_bonded_amount", share_asset_to_bond.amount))
}

fn callback_unbond(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    bond_units_to_deduct: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // Query how many share tokens is currently being bonded by us
    let (total_bonded_amount, _) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // Calculate how many share tokens to unbond according the `bond_units_to_deduct`
    let amount_to_unbond =
        total_bonded_amount.multiply_ratio(bond_units_to_deduct, state.total_bond_units);

    state.total_bond_units -= bond_units_to_deduct;
    position.bond_units -= bond_units_to_deduct;

    add_unlocked_asset(&mut position, &Asset::cw20(&config.pair.liquidity_token, amount_to_unbond));

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.staking.unbond_msg(amount_to_unbond)?)
        .add_attribute("action", "martian_field :: callback :: unbond")
        .add_attribute("bond_units_deducted", bond_units_to_deduct)
        .add_attribute("share_unbonded_amount", amount_to_unbond))
}

fn callback_borrow(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    borrow_amount: Uint128,
) -> StdResult<Response> {
    // If borrow amount is zero, we do nothing
    if borrow_amount.is_zero() {
        return Ok(Response::default());
    }

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

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

    add_unlocked_asset(&mut position, &secondary_asset_to_add);

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.borrow_msg(&secondary_asset_to_borrow)?)
        .add_attribute("action", "martian_field :: callback :: borrow")
        .add_attribute("debt_units_added", debt_units_to_add)
        .add_attribute("secondary_borrowed_amount", borrow_amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}

fn callback_repay(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    repay_amount: Uint128,
) -> StdResult<Response> {
    // If repay amount is zero, we do nothing
    if repay_amount.is_zero() {
        return Ok(Response::default());
    }

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

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

    deduct_unlocked_asset(&mut position, &secondary_asset_to_deduct)?;

    STATE.save(deps.storage, &state)?;
    POSITION.save(deps.storage, &user_addr, &position)?;

    Ok(Response::new()
        .add_message(config.red_bank.repay_msg(&secondary_asset_to_repay)?)
        .add_attribute("action", "martian_field :: callback :: repay")
        .add_attribute("debt_units_deducted", debt_units_to_deduct)
        .add_attribute("secondary_repaid_amount", secondary_asset_to_repay.amount)
        .add_attribute("secondary_deducted_amount", secondary_asset_to_deduct.amount))
}

fn callback_swap(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    swap_amount: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // If swap amount is not provided, we use all available unlocked amount, after tax
    let swap_amount = swap_amount.unwrap_or_else(|| {
        find_unlocked_asset(&position, &config.primary_asset_info)
            .deduct_tax(&deps.querier)
            .unwrap()
            .amount
    });

    // If swap amount is zero, we do nothing
    if swap_amount.is_zero() {
        return Ok(Response::default());
    }

    // NOTE: `swap_amount` is the amount to be delivered to the AMM. The total cost of making this
    // transfer is `swap_amount` plus tax. We deduct this amount from the user's unllocked primary asset
    let primary_asset_to_offer = Asset::new(&config.primary_asset_info, swap_amount);
    let primary_asset_to_deduct = primary_asset_to_offer.add_tax(&deps.querier)?;

    deduct_unlocked_asset(&mut position, &primary_asset_to_deduct)?;

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.save(deps.storage, &user_addr)?;

    Ok(Response::new()
        .add_submessage(config.pair.swap_submsg(2, &primary_asset_to_offer)?)
        .add_attribute("action", "martian_field :: callback :: swap")
        .add_attribute("primary_offered_amount", primary_asset_to_offer.amount)
        .add_attribute("primary_deducted_amount", primary_asset_to_deduct.amount))
}

fn callback_refund(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    recipient_addr: Addr,
    percentage: Decimal,
) -> StdResult<Response> {
    let mut position = POSITION.load(deps.storage, &user_addr)?;

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
        deduct_unlocked_asset(&mut position, asset)?;
    }

    POSITION.save(deps.storage, &user_addr, &position)?;

    let msgs: Vec<CosmosMsg> =
        assets_to_refund.iter().map(|asset| asset.transfer_msg(&recipient_addr).unwrap()).collect();

    let refund_attrs: Vec<Attribute> = assets_to_refund
        .iter()
        .map(|asset| attr("asset_refunded", format!("{}{}", asset.amount, asset.info.get_denom())))
        .collect();
    let deduct_attrs: Vec<Attribute> = assets_to_deduct
        .iter()
        .map(|asset| attr("asset_deducted", format!("{}{}", asset.amount, asset.info.get_denom())))
        .collect();

    Ok(Response::new()
        .add_messages(msgs)
        .add_attribute("action", "martian_field :: callback :: refund")
        .add_attributes(refund_attrs)
        .add_attributes(deduct_attrs))
}

fn callback_assert_health(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr)?;
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
        format!("{}", ltv)
    } else {
        "null".to_string()
    };

    if !healthy {
        return Err(StdError::generic_err(format!("ltv greater than threshold: {}", ltv_str)));
    }

    let event = Event::new("position_changed")
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

fn callback_snapshot(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr)?;
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

//--------------------------------------------------------------------------------------------------
// Replies
//--------------------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> StdResult<Response> {
    match reply.id {
        // After providing liquidity, find out how many share tokens were minted, and increment unlocked
        // amount in the user's position
        0 => reply_after_provide_liquidity(deps, env, reply.result.unwrap()),
        // After withdrawing liquidity, find out how much each of the assets were returned, and increment
        // unlocked amounts in the user's position
        1 => reply_after_withdraw_liquidity(deps, env, reply.result.unwrap()),
        // After swapping, find out how much asset was returned, and increment unlocked amounts in the
        // user's position
        2 => reply_after_swap(deps, env, reply.result.unwrap()),
        // Other IDs are invalid
        id => Err(StdError::generic_err(format!("invalid id: {}", id))),
    }
}

fn reply_after_provide_liquidity(
    deps: DepsMut,
    _env: Env,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = TEMP_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let share_minted_amount = Pair::parse_provide_events(&response.events)?;
    let shares_to_add = Asset::cw20(&config.pair.liquidity_token, share_minted_amount);

    add_unlocked_asset(&mut position, &shares_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_provide_liquidity")
        .add_attribute("user_addr", user_addr)
        .add_attribute("share_added_amount", shares_to_add.amount))
}

fn reply_after_withdraw_liquidity(
    deps: DepsMut,
    _env: Env,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = TEMP_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let (primary_asset_withdrawn, secondary_asset_withdrawn) = Pair::parse_withdraw_events(
        &response.events,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;

    // The withdrawn amounts returned in Astroport's response event are the pre-tax amounts. We need
    // to deduct tax to find the amounts we actually received. We add the after-tax amounts to the
    // user's unlocked assets
    let primary_asset_to_add = primary_asset_withdrawn.deduct_tax(&deps.querier)?;
    let secondary_asset_to_add = secondary_asset_withdrawn.deduct_tax(&deps.querier)?;

    add_unlocked_asset(&mut position, &primary_asset_to_add);
    add_unlocked_asset(&mut position, &secondary_asset_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_withdraw_liquidity")
        .add_attribute("user_addr", user_addr)
        .add_attribute("primary_withdrawn_amount", primary_asset_withdrawn.amount)
        .add_attribute("primary_added_amount", primary_asset_to_add.amount)
        .add_attribute("secondary_withdrawn_amount", secondary_asset_withdrawn.amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}

fn reply_after_swap(
    deps: DepsMut,
    _env: Env,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = TEMP_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let secondary_asset_returned_amount = Pair::parse_swap_events(&response.events)?;
    let secondary_asset_returned =
        Asset::new(&config.secondary_asset_info, secondary_asset_returned_amount);

    // The return amount returned in Astroport's response event is the pre-tax amount. We need to
    // deduct tax to find the amount we actually received. We add the after-tax amount to the user's
    // unlocked asset
    let secondary_asset_to_add = secondary_asset_returned.deduct_tax(&deps.querier)?;

    add_unlocked_asset(&mut position, &secondary_asset_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    TEMP_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_swap")
        .add_attribute("user_addr", user_addr)
        .add_attribute("secondary_returned_amount", secondary_asset_returned.amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}

//--------------------------------------------------------------------------------------------------
// Queries
//--------------------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps, env)?),
        QueryMsg::State {} => to_binary(&query_state(deps, env)?),
        QueryMsg::Position {
            user,
        } => to_binary(&query_position(deps, env, user)?),
        QueryMsg::Health {
            user,
        } => to_binary(&query_health(deps, env, user)?),
        QueryMsg::Snapshot {
            user,
        } => to_binary(&query_snapshot(deps, env, user)?),
    }
}

fn query_config(deps: Deps, _env: Env) -> StdResult<ConfigUnchecked> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.into())
}

fn query_state(deps: Deps, _env: Env) -> StdResult<State> {
    STATE.load(deps.storage)
}

fn query_position(deps: Deps, _env: Env, user: String) -> StdResult<PositionUnchecked> {
    let user_addr = deps.api.addr_validate(&user)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    Ok(position.into())
}

fn query_health(deps: Deps, env: Env, user: String) -> StdResult<Health> {
    let user_addr = deps.api.addr_validate(&user)?;
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    compute_health(&deps.querier, &env, &config, &state, &position)
}

fn query_snapshot(deps: Deps, _env: Env, user: String) -> StdResult<Snapshot> {
    let user_addr = deps.api.addr_validate(&user)?;
    SNAPSHOT.load(deps.storage, &user_addr)
}

//--------------------------------------------------------------------------------------------------
// Migration
//--------------------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new()) // do nothing
}
