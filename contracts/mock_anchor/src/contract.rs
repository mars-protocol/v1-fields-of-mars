#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_binary, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use field_of_mars::staking::anchor_staking::{
    Cw20HookMsg, ExecuteMsg, MockInstantiateMsg, QueryMsg, StakerInfoResponse,
};

use crate::state::{CONFIG, POSITION};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: MockInstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(deps.storage, &msg)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => _receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::Unbond {
            amount,
        } => unbond(deps, env, info, amount),
        ExecuteMsg::Withdraw {} => withdraw(deps, env, info),
    }
}

fn _receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => {
            if info.sender == config.staking_token {
                bond(deps, env, cw20_msg.sender, cw20_msg.amount)
            } else {
                Err(StdError::generic_err("only ANC-UST LP token can be bonded"))
            }
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::StakerInfo {
            staker,
            ..
        } => to_binary(&query_staker_info(deps, env, staker)?),
    }
}

//----------------------------------------------------------------------------------------
// Execute Functions
//----------------------------------------------------------------------------------------

fn bond(
    deps: DepsMut,
    _env: Env,
    staker: String,
    amount: Uint128,
) -> StdResult<Response> {
    let staker_addr = deps.api.addr_validate(&staker)?;
    let mut position = POSITION.load(deps.storage, &staker_addr).unwrap_or_default();

    position.bond_amount += amount;
    POSITION.save(deps.storage, &staker_addr, &position)?;

    Ok(Response::default())
}

fn unbond(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &info.sender)?;

    position.bond_amount = position.bond_amount.checked_sub(amount)?;
    POSITION.save(deps.storage, &info.sender, &position)?;

    Ok(Response::new().add_submessage(SubMsg::new(WasmMsg::Execute {
        contract_addr: config.staking_token,
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: String::from(info.sender),
            amount,
        })?,
        funds: vec![],
    })))
}

fn withdraw(deps: DepsMut, _env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    Ok(Response::new().add_submessage(SubMsg::new(WasmMsg::Execute {
        contract_addr: config.anchor_token,
        msg: to_binary(&Cw20ExecuteMsg::Transfer {
            recipient: String::from(info.sender),
            amount: Uint128::new(1000000u128), // 1.0 ANC
        })?,
        funds: vec![],
    })))
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

fn query_staker_info(
    deps: Deps,
    _env: Env,
    staker: String,
) -> StdResult<StakerInfoResponse> {
    let staker_addr = deps.api.addr_validate(&staker)?;
    let position = POSITION.load(deps.storage, &staker_addr).unwrap_or_default();
    Ok(StakerInfoResponse {
        staker,
        reward_index: Decimal::zero(),
        bond_amount: position.bond_amount,
        pending_reward: Uint128::new(1000000u128), // 1.0 ANC
    })
}
