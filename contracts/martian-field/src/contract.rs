use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Reply, Response, StdError,
    StdResult,
};

use fields_of_mars::martian_field::msg::{
    CallbackMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg,
};

use crate::helpers::unwrap_reply;
use crate::{execute, execute_callbacks as callbacks, execute_replies as replies, queries};

#[entry_point]
pub fn instantiate(deps: DepsMut, _env: Env, _info: MessageInfo, msg: InstantiateMsg) -> StdResult<Response> {
    let config = msg.check(deps.api)?;
    config.validate()?;
    execute::init_storage(deps, config)
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    let api = deps.api;
    match msg {
        ExecuteMsg::UpdatePosition(actions) => execute::update_position(deps, env, info, actions),
        ExecuteMsg::Harvest {
            max_spread,
            slippage_tolerance,
        } => execute::harvest(deps, env, max_spread, slippage_tolerance),
        ExecuteMsg::Liquidate {
            user,
        } => execute::liquidate(deps, env, info, api.addr_validate(&user)?),
        ExecuteMsg::UpdateConfig {
            new_config,
        } => execute::update_config(deps, info, new_config.check(api)?),
        ExecuteMsg::Callback(callback_msg) => execute_callback(deps, env, info, callback_msg),
    }
}

fn execute_callback(deps: DepsMut, env: Env, info: MessageInfo, msg: CallbackMsg) -> StdResult<Response> {
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }
    match msg {
        CallbackMsg::ProvideLiquidity {
            user_addr,
            slippage_tolerance,
        } => callbacks::provide_liquidity(deps, user_addr, slippage_tolerance),
        CallbackMsg::WithdrawLiquidity {
            user_addr,
        } => callbacks::withdraw_liquidity(deps, user_addr),
        CallbackMsg::Bond {
            user_addr,
        } => callbacks::bond(deps, env, user_addr),
        CallbackMsg::Unbond {
            user_addr,
            bond_units_to_reduce,
        } => callbacks::unbond(deps, env, user_addr, bond_units_to_reduce),
        CallbackMsg::Borrow {
            user_addr,
            borrow_amount,
        } => callbacks::borrow(deps, env, user_addr, borrow_amount),
        CallbackMsg::Repay {
            user_addr,
            repay_amount,
        } => callbacks::repay(deps, env, user_addr, repay_amount),
        CallbackMsg::Refund {
            user_addr,
            recipient_addr,
            percentage,
        } => callbacks::refund(deps, user_addr, recipient_addr, percentage),
        CallbackMsg::Swap {
            user_addr,
            offer_asset_info,
            offer_amount,
            max_spread,
        } => callbacks::swap(deps, user_addr, offer_asset_info, offer_amount, max_spread),
        CallbackMsg::Balance {
            max_spread,
        } => callbacks::balance(deps, env, max_spread),
        CallbackMsg::Cover {
            user_addr,
        } => callbacks::cover(deps, env, user_addr),
        CallbackMsg::AssertHealth {
            user_addr,
        } => callbacks::assert_health(deps, env, user_addr),
        CallbackMsg::Snapshot {
            user_addr,
        } => callbacks::snapshot(deps, env, user_addr),
    }
}

#[entry_point]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> StdResult<Response> {
    match reply.id {
        0 => replies::after_provide_liquidity(deps, unwrap_reply(reply)?),
        1 => replies::after_withdraw_liquidity(deps, unwrap_reply(reply)?),
        2 => replies::after_swap(deps, unwrap_reply(reply)?),
        id => Err(StdError::generic_err(format!("invalid reply id: {}", id))),
    }
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&queries::query_config(deps, env)?),
        QueryMsg::State {} => to_binary(&queries::query_state(deps, env)?),
        QueryMsg::Position {
            user,
        } => to_binary(&queries::query_position(deps, env, user)?),
        QueryMsg::Health {
            user,
        } => to_binary(&queries::query_health(deps, env, user)?),
        QueryMsg::Snapshot {
            user,
        } => to_binary(&queries::query_snapshot(deps, user)?),
    }
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new()) // do nothing
}
