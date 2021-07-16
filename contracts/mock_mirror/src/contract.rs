use cosmwasm_std::{
    from_binary, to_binary, Api, Binary, CosmosMsg, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};

use fields_of_mars::staking::mirror_staking::{
    Cw20HookMsg, HandleMsg, MockInitMsg, QueryMsg, RewardInfoResponse,
    RewardInfoResponseItem,
};

use crate::state::{
    read_config, read_reward_info, write_config, write_reward_info, Config,
};

//----------------------------------------------------------------------------------------
// ENTRY POINTS
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MockInitMsg,
) -> StdResult<InitResponse> {
    write_config(
        &mut deps.storage,
        &Config {
            mirror_token: deps.api.canonical_address(&msg.mirror_token)?,
            asset_token: deps.api.canonical_address(&msg.asset_token)?,
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
        HandleMsg::Receive(msg) => _receive_cw20(deps, env, msg),
        HandleMsg::Unbond {
            asset_token: _,
            amount,
        } => unbond(deps, env, amount),
        HandleMsg::Withdraw {
            asset_token: _,
        } => withdraw(deps, env),
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
            Cw20HookMsg::Bond {
                ..
            } => {
                if env.message.sender == staking_token {
                    bond(deps, env, cw20_msg.sender, cw20_msg.amount)
                } else {
                    Err(StdError::generic_err("only MIR-UST LP token can be bonded"))
                }
            }
        }
    } else {
        Err(StdError::generic_err("data not given"))
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::RewardInfo {
            staker_addr,
            ..
        } => to_binary(&query_reward_info(deps, staker_addr)?),
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
    let mut reward_info = read_reward_info(&deps.storage, &staker_raw)?;

    reward_info.bond_amount += amount;
    write_reward_info(&mut deps.storage, &staker_raw, &reward_info)?;

    Ok(HandleResponse::default())
}

pub fn unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let staker_raw = &deps.api.canonical_address(&env.message.sender)?;
    let config = read_config(&deps.storage)?;
    let mut reward_info = read_reward_info(&deps.storage, &staker_raw)?;

    reward_info.bond_amount = (reward_info.bond_amount - amount)?;
    write_reward_info(&mut deps.storage, &staker_raw, &reward_info)?;

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
            contract_addr: deps.api.human_address(&config.mirror_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount: Uint128(1000000), // 1.0 MIR
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

pub fn query_reward_info<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    staker: HumanAddr,
) -> StdResult<RewardInfoResponse> {
    let staker_raw = deps.api.canonical_address(&staker)?;
    let config = read_config(&deps.storage)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;
    let reward_info = read_reward_info(&deps.storage, &staker_raw)?;

    let reward_infos = if !reward_info.bond_amount.is_zero() {
        vec![RewardInfoResponseItem {
            asset_token,
            bond_amount: reward_info.bond_amount,
            pending_reward: Uint128(1000000),
            is_short: false,
        }]
    } else {
        vec![]
    };

    Ok(RewardInfoResponse {
        staker_addr: staker,
        reward_infos,
    })
}
