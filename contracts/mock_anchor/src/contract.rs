use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;

use fields_of_mars::adapters::Asset;

use anchor_token::staking::{Cw20HookMsg, ExecuteMsg, QueryMsg, StakerInfoResponse};

use crate::msg::InstantiateMsg;
use crate::state::{Config, BOND_AMOUNT, CONFIG};

static MOCK_REWARD_AMOUNT: u128 = 1000000;

// INIT

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        anchor_token: deps.api.addr_validate(&msg.anchor_token)?,
        staking_token: deps.api.addr_validate(&msg.staking_token)?,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

// EXECUTE

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::Unbond { amount } => execute_unbond(deps, env, info, amount),
        ExecuteMsg::Withdraw {} => execute_withdraw(deps, env, info),

        _ => Err(StdError::generic_err("Unimplemented")),
    }
}

fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond {} => {
            if info.sender != config.staking_token {
                return Err(StdError::generic_err("unauthorized"));
            }

            execute_bond(deps, env, info, cw20_msg.sender, cw20_msg.amount)
        }
    }
}

fn execute_bond(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    staker: String,
    amount: Uint128,
) -> StdResult<Response> {
    let staker_addr = deps.api.addr_validate(&staker)?;

    let bond_amount = helpers::load_bond_amount(deps.storage, &staker_addr);

    BOND_AMOUNT.save(deps.storage, &staker_addr, &(bond_amount + amount))?;

    Ok(Response::default())
}

fn execute_unbond(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let bond_amount = helpers::load_bond_amount(deps.storage, &info.sender);

    BOND_AMOUNT.save(deps.storage, &info.sender, &(bond_amount - amount))?;

    let outbound_asset = Asset::cw20(&config.staking_token, amount);
    let outbound_msg = outbound_asset.transfer_msg(&info.sender)?;

    Ok(Response::new().add_message(outbound_msg))
}

fn execute_withdraw(deps: DepsMut, _env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let outbound_asset = Asset::cw20(&config.anchor_token, MOCK_REWARD_AMOUNT);
    let outbound_msg = outbound_asset.transfer_msg(&info.sender)?;

    Ok(Response::new().add_message(outbound_msg))
}

// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::StakerInfo { staker, .. } => to_binary(&query_staker_info(deps, env, staker)?),

        _ => Err(StdError::generic_err("Unimplemented")),
    }
}

fn query_staker_info(deps: Deps, _env: Env, staker: String) -> StdResult<StakerInfoResponse> {
    let staker_addr = deps.api.addr_validate(&staker)?;

    let bond_amount = helpers::load_bond_amount(deps.storage, &staker_addr);

    Ok(StakerInfoResponse {
        staker,
        reward_index: Decimal::zero(),
        bond_amount,
        pending_reward: Uint128::new(MOCK_REWARD_AMOUNT),
    })
}

mod helpers {
    use cosmwasm_std::Storage;

    use super::*;

    pub fn load_bond_amount(storage: &dyn Storage, staker_addr: &Addr) -> Uint128 {
        BOND_AMOUNT
            .load(storage, staker_addr)
            .unwrap_or_else(|_| Uint128::zero())
    }
}
