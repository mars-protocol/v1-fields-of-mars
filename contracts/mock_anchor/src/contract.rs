use anchor_token::staking::{
    Cw20HookMsg, HandleMsg, InitMsg, QueryMsg, StakerInfoResponse,
};
use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

use crate::state::{
    read_config, read_staker_info, write_config, write_staker_info, Config,
};

//----------------------------------------------------------------------------------------
// ENTRY POINTS
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    write_config(
        &mut deps.storage,
        &Config {
            anchor_token: deps.api.canonical_address(&msg.anchor_token)?,
            staking_token: deps.api.canonical_address(&msg.staking_token)?,
        },
    )?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(cw20_msg) => _receive_cw20(deps, env, cw20_msg),
        HandleMsg::Unbond {
            amount,
        } => unbond(deps, env, amount),
        HandleMsg::Withdraw {} => withdraw(deps, env),
    }
}

pub fn _receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<HandleResponse> {
    if let Some(msg) = cw20_msg.msg {
        let config = read_config(&deps.storage)?;
        let staking_token = deps.api.human_address(&config.staking_token)?;
        match from_binary(&msg)? {
            Cw20HookMsg::Bond {} => {
                if env.message.sender == staking_token {
                    bond(deps, env, cw20_msg.sender, cw20_msg.amount)
                } else {
                    Err(StdError::generic_err("only ANC-UST LP token can be bonded"))
                }
            }
        }
    } else {
        Err(StdError::generic_err("data should be given"))
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::StakerInfo {
            staker,
            ..
        } => to_binary(&query_staker_info(deps, staker)?),
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

//----------------------------------------------------------------------------------------
// HANDLE FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    staker: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;
    let mut staker_info = read_staker_info(&deps.storage, &staker_raw)?;

    staker_info.bond_amount += amount;
    write_staker_info(&mut deps.storage, &staker_raw, &staker_info)?;

    Ok(HandleResponse::default())
}

pub fn unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let staker_raw = deps.api.canonical_address(&env.message.sender)?;
    let config = read_config(&deps.storage)?;
    let mut staker_info = read_staker_info(&deps.storage, &staker_raw)?;

    staker_info.bond_amount = (staker_info.bond_amount - amount)?;
    write_staker_info(&mut deps.storage, &staker_raw, &staker_info)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.staking_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount,
            })?,
            send: vec![],
        })],
        log: vec![],
        data: None,
    })
}

pub fn withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.anchor_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount: Uint128(1000000), // 1.0 ANC
            })?,
            send: vec![],
        })],
        log: vec![],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// QUERY FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn query_staker_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    staker: HumanAddr,
) -> StdResult<StakerInfoResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;
    let staker_info = read_staker_info(&deps.storage, &staker_raw)?;
    Ok(StakerInfoResponse {
        staker,
        reward_index: Decimal::zero(),
        bond_amount: staker_info.bond_amount,
        pending_reward: Uint128(1000000), // 1.0 ANC
    })
}
