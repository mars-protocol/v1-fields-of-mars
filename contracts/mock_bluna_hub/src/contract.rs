use anchor_basset_hub::{
    CurrentBatchResponse, HandleMsg, ParametersResponse, QueryMsg, StateResponse,
};
use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, Querier, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20HandleMsg;
use terraswap::querier::query_supply;

use crate::{
    math::decimal_division,
    msg::InitMsg,
    state::{read_config, read_state, write_config, write_state, Config, State},
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
            token_contract: CanonicalAddr::default(), // to be updated later
            exchange_rate: msg.exchange_rate,
            er_threshold: msg.er_threshold,
            peg_recovery_fee: msg.peg_recovery_fee,
            requested_with_fee: msg.requested_with_fee,
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
        HandleMsg::Bond {
            ..
        } => bond(deps, env),
        HandleMsg::UpdateConfig {
            token_contract,
            ..
        } => update_config(deps, env, token_contract.unwrap()),
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Parameters {} => to_binary(&query_parameters(deps)?),
        QueryMsg::CurrentBatch {} => to_binary(&query_current_batch(&deps)?),
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

//----------------------------------------------------------------------------------------
// HANDLE FUNCTIONS
//----------------------------------------------------------------------------------------

/**
 * @dev Forked from
 * https://github.com/Anchor-Protocol/anchor-bAsset-contracts/blob/master/contracts/anchor_basset_hub/src/bond.rs#L12
 */
pub fn bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let state = read_state(&deps.storage)?;
    let token_contract = deps.api.human_address(&config.token_contract)?;
    let total_supply = query_supply(deps, &token_contract)?;

    // Find the amount of LUNA shelpers
    let payment =
        env.message.sent_funds.iter().find(|x| x.denom == "uluna").ok_or_else(|| {
            StdError::generic_err("No uluna assets are provided to bond")
        })?;

    // Calculate the amount of bAsset to mint
    let mint_amount = decimal_division(payment.amount, config.exchange_rate);

    // If bLUNA:LUNA ratio is off-peg, calculate the peg fee
    let mint_amount_with_fee = if config.exchange_rate < config.er_threshold {
        let max_peg_fee = mint_amount * config.peg_recovery_fee;
        let required_peg_fee =
            ((total_supply + mint_amount + config.requested_with_fee)
                - (state.total_bond_amount + payment.amount))?;
        let peg_fee = Uint128::min(max_peg_fee, required_peg_fee);
        (mint_amount - peg_fee)?
    } else {
        mint_amount
    };

    write_state(
        &mut deps.storage,
        &State {
            total_bond_amount: state.total_bond_amount + payment.amount,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: token_contract,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Mint {
                recipient: env.message.sender.clone(),
                amount: mint_amount_with_fee,
            })?,
        })],
        log: vec![
            log("action", "mint"),
            log("from", env.message.sender),
            log("bonded", payment.amount),
            log("minted", mint_amount_with_fee),
        ],
        data: None,
    })
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    token_contract: HumanAddr,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    write_config(
        &mut deps.storage,
        &Config {
            token_contract: deps.api.canonical_address(&token_contract)?,
            exchange_rate: config.exchange_rate,
            er_threshold: config.er_threshold,
            peg_recovery_fee: config.peg_recovery_fee,
            requested_with_fee: config.requested_with_fee,
        },
    )?;
    Ok(HandleResponse::default())
}

//----------------------------------------------------------------------------------------
// QUERY FUNCTIONS
//----------------------------------------------------------------------------------------

fn query_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<StateResponse> {
    Ok(StateResponse {
        exchange_rate: read_config(&deps.storage)?.exchange_rate,
        total_bond_amount: read_state(&deps.storage)?.total_bond_amount,
        // The other parameters don't matter; return zero
        last_index_modification: 0u64,
        prev_hub_balance: Uint128::zero(),
        actual_unbonded_amount: Uint128::zero(),
        last_unbonded_time: 0u64,
        last_processed_batch: 0u64,
    })
}

fn query_parameters<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ParametersResponse> {
    let config = read_config(&deps.storage)?;
    Ok(ParametersResponse {
        er_threshold: config.er_threshold,
        peg_recovery_fee: config.peg_recovery_fee,
        // The othe parameters don't matter; return zero
        epoch_period: 0u64,
        unbonding_period: 0u64,
        underlying_coin_denom: "ngmi".to_string(),
        reward_denom: "hfsp".to_string(),
    })
}

fn query_current_batch<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<CurrentBatchResponse> {
    Ok(CurrentBatchResponse {
        requested_with_fee: read_config(&deps.storage)?.requested_with_fee,
        // The other parameters don't matter; return zero
        id: 0u64,
    })
}
