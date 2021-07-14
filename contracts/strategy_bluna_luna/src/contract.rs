use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, Coin, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, MigrateResponse, Querier, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20HandleMsg;
use mars::red_bank;
use terraswap::{
    asset::{Asset, AssetInfo},
    querier::{query_balance, query_supply, query_token_balance},
};

use fields_of_mars::strategy_bluna_luna::{
    CallbackMsg, ConfigResponse, HandleMsg, InitMsg, MigrateMsg, PositionResponse,
    QueryMsg, StateResponse,
};

use crate::{
    helpers,
    state::{
        delete_position, read_config, read_position, read_state, write_config,
        write_position, write_state, Config, Position, State,
    },
};

//----------------------------------------------------------------------------------------
// ENTRY POINTS
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    write_state(
        &mut deps.storage,
        &State::new(deps.api.canonical_address(&env.contract.address)?),
    )?;
    Ok(InitResponse {
        messages: vec![CallbackMsg::UpdateConfig {
            new_config: msg,
        }
        .into_cosmos_msg(&env.contract.address)?],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::IncreasePosition {
            bluna_amount,
        } => increase_position(deps, env, bluna_amount),
        HandleMsg::ReducePosition {
            pool_units,
        } => reduce_position(deps, env, pool_units),
        HandleMsg::PayDebt {
            user,
        } => pay_debt(deps, env, user),
        HandleMsg::Liquidate {
            user,
        } => liquidate(deps, env, user),
        HandleMsg::UpdateConfig {
            new_config,
        } => update_config(deps, env, new_config),
        HandleMsg::Callback(callback_msg) => _handle_callback(deps, env, callback_msg),
    }
}

pub fn _handle_callback<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    callback_msg: CallbackMsg,
) -> StdResult<HandleResponse> {
    // Callback functions can only be called this contract itself
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
    }
    match callback_msg {
        CallbackMsg::ProvideLiquidity {
            user,
            luna_amount,
            bluna_amount,
        } => _provide_liquidity(deps, env, user, luna_amount, bluna_amount),
        CallbackMsg::RemoveLiquidity {
            user,
            pool_units,
        } => _remove_liquidity(deps, env, user, pool_units),
        CallbackMsg::Borrow {
            user,
            borrow_amount,
        } => _borrow(deps, env, user, borrow_amount),
        CallbackMsg::Repay {
            user,
            repay_amount,
        } => _repay(deps, env, user, repay_amount),
        CallbackMsg::Refund {
            user,
        } => _refund(deps, env, user),
        CallbackMsg::ClaimCollateral {
            user,
            liquidator,
            repay_amount,
        } => _claim_collateral(deps, env, user, liquidator, repay_amount),
        CallbackMsg::UpdateConfig {
            new_config,
        } => _update_config(deps, env, new_config),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Position {
            user,
        } => to_binary(&query_position(deps, user)?),
    }
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<MigrateResponse> {
    Err(StdError::generic_err("unimplemented"))
}

//----------------------------------------------------------------------------------------
// HANDLE FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn increase_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    bluna_to_draw: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn reduce_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    pool_units: Option<Uint128>,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn pay_debt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn liquidate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    new_config: InitMsg,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

//----------------------------------------------------------------------------------------
// CALLBACK FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn _provide_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    luna_amount: Uint128,
    bluna_amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _remove_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    pool_units: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _borrow<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    borrow_amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _repay<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    repay_amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _claim_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    liquidator: HumanAddr,
    repay_amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn _update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    config: InitMsg,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

//----------------------------------------------------------------------------------------
// QUERY FUNCTIONS
//----------------------------------------------------------------------------------------

/**
 * @notice Return strategy configurations.
 */
pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let config = read_config(&deps.storage)?;
    Ok(ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        treasury: deps.api.human_address(&config.treasury)?,
        bluna_hub: deps.api.human_address(&config.bluna_hub)?,
        bluna_token: deps.api.human_address(&config.bluna_token)?,
        bluna_validator: deps.api.human_address(&config.bluna_validator)?,
        pool: deps.api.human_address(&config.pool)?,
        pool_token: deps.api.human_address(&config.pool_token)?,
        red_bank: deps.api.human_address(&config.red_bank)?,
        liquidation_fee_rate: config.liquidation_fee_rate,
        liquidation_threshold: config.liquidation_threshold,
    })
}

/**
 * @notice Return the global state of the strategy.
 */
pub fn query_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<StateResponse> {
    let state = read_state(&deps.storage)?;
    Ok(StateResponse {
        total_pool_value: Uint128::default(),
        total_pool_units: state.total_pool_units,
        total_debt_value: Uint128::default(),
        total_debt_units: state.total_debt_units,
        utilization: Some(Decimal::default()),
    })
}

/**
 * @notice Return data on an individual user's position.
 */
pub fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: HumanAddr,
) -> StdResult<PositionResponse> {
    let position = read_position(&deps.storage, &&deps.api.canonical_address(&user)?)?;
    Ok(PositionResponse {
        is_active: position.is_active(),
        pool_value: Uint128::default(),
        pool_units: position.pool_units,
        debt_value: Uint128::default(),
        debt_units: position.debt_units,
        utilization: Some(Decimal::default()),
        unlocked_luna_amount: position.unlocked_luna_amount,
        unlocked_bluna_amount: position.unlocked_bluna_amount,
    })
}
