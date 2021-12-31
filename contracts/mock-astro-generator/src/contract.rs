use cosmwasm_std::{
    entry_point, from_binary, to_binary, to_vec, Addr, Binary, CosmosMsg, Deps, DepsMut, Env,
    MessageInfo, Response, StdError, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;

use astroport::generator::{
    Cw20HookMsg, ExecuteMsg, PendingTokenResponse, QueryMsg, RewardInfoResponse
};

use cw_asset::Asset;

use crate::msg::{Config, InstantiateMsg};
use crate::state::{CONFIG, DEPOSIT};

static MOCK_ASTRO_REWARD_AMOUNT: Uint128 = Uint128::new(1000000); // 1 ASTRO
static MOCK_PROXY_REWARD_AMOUNT: Uint128 = Uint128::new(500000); // 0.5 MIR or whatever token

// INIT

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(deps.storage, &msg)?;
    Ok(Response::default())
}

// EXECUTE

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, info, cw20_msg),

        ExecuteMsg::Withdraw {
            lp_token,
            amount,
        } => execute_withdraw(deps, info.sender, lp_token, amount),

        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented execute: {}", String::from_utf8(to_vec(&msg)?)?)
        )),
    }
}

fn execute_receive_cw20(
    deps: DepsMut,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    let api = deps.api;
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Deposit {} => execute_deposit(
            deps,
            api.addr_validate(&cw20_msg.sender)?,
            info.sender,
            cw20_msg.amount,
        ),

        Cw20HookMsg::DepositFor(user_addr) => {
            execute_deposit(deps, user_addr, info.sender, cw20_msg.amount)
        }
    }
}

fn execute_deposit(
    deps: DepsMut,
    user_addr: Addr,
    liquidity_token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if liquidity_token != config.liquidity_token {
        return Err(StdError::generic_err(
            format!(
                "[mock] invalid liquidity token! expected: {}, received: {}",
                config.liquidity_token, 
                liquidity_token
            )
        ));
    }

    let mut deposit = DEPOSIT.load(deps.storage, &user_addr).unwrap_or_else(|_| Uint128::zero());
    deposit = deposit.checked_add(amount)?;
    DEPOSIT.save(deps.storage, &user_addr, &deposit)?;

    // reward is automatically withdrawn every time a deposit is made
    Ok(Response::new().add_messages(_withdraw_reward_messages(&config, &user_addr)?))
}

fn execute_withdraw(
    deps: DepsMut,
    user_addr: Addr,
    liquidity_token: Addr,
    amount: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if liquidity_token != config.liquidity_token {
        return Err(StdError::generic_err(
            format!(
                "[mock] invalid liquidity token! expected: {}, received: {}",
                config.liquidity_token, 
                liquidity_token
            )
        ));
    }

    let mut deposit = DEPOSIT.load(deps.storage, &user_addr).unwrap_or_else(|_| Uint128::zero());
    deposit = deposit.checked_sub(amount)?;
    DEPOSIT.save(deps.storage, &user_addr, &deposit)?;

    let mut res = Response::new();

    // withdraw liquidity tokens
    if !amount.is_zero() {
        let liquidity_token_to_withdraw = Asset::cw20(config.liquidity_token.clone(), amount);
        res = res.add_message(liquidity_token_to_withdraw.transfer_msg(&user_addr)?);
    }

    // reward is automatically withdrawn every time a withdrawal is made
    Ok(res.add_messages(_withdraw_reward_messages(&config, &user_addr)?))
}

fn _withdraw_reward_messages(config: &Config, user_addr: &Addr) -> StdResult<Vec<CosmosMsg>> {
    let mut msgs: Vec<CosmosMsg> = vec![];

    // send Astro reward
    let astro_token_to_send = Asset::cw20(config.astro_token.clone(), MOCK_ASTRO_REWARD_AMOUNT);
    msgs.push(astro_token_to_send.transfer_msg(user_addr)?);

    // if proxy reward token is specified, send proxy reward
    if let Some(proxy_reward_token) = &config.proxy_reward_token {
        let proxy_token_to_send = Asset::cw20(proxy_reward_token.clone(), MOCK_PROXY_REWARD_AMOUNT);
        msgs.push(proxy_token_to_send.transfer_msg(user_addr)?);
    }

    Ok(msgs)
}

// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::RewardInfo {
            ..
        } => to_binary(&query_reward_info(deps)?),
        QueryMsg::Deposit {
            lp_token,
            user,
        } => to_binary(&query_deposit(deps, lp_token, user)?),
        QueryMsg::PendingToken {
            lp_token,
            user: _, // this mock contract returns fixed amount of rewards regardless of user deposit
        } => to_binary(&query_pending_token(deps, lp_token)?),

        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented query: {}", String::from_utf8(to_vec(&msg)?)?)
        )),
    }
}

fn query_reward_info(deps: Deps) -> StdResult<RewardInfoResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(RewardInfoResponse {
        base_reward_token: config.astro_token,
        proxy_reward_token: config.proxy_reward_token
    })
}

fn query_deposit(deps: Deps, liquidity_token: Addr, user_addr: Addr) -> StdResult<Uint128> {
    let config = CONFIG.load(deps.storage)?;
    if liquidity_token != config.liquidity_token {
        return Ok(Uint128::zero());
    }

    let deposit = DEPOSIT.load(deps.storage, &user_addr).unwrap_or_else(|_| Uint128::zero());
    Ok(deposit)
}

fn query_pending_token(deps: Deps, liquidity_token: Addr) -> StdResult<PendingTokenResponse> {
    let config = CONFIG.load(deps.storage)?;

    let pending = if liquidity_token == config.liquidity_token {
        MOCK_ASTRO_REWARD_AMOUNT
    } else {
        return Err(StdError::generic_err(
            format!(
                "[mock] invalid liquidity token! expected: {}, received: {}",
                config.liquidity_token, 
                liquidity_token
            )
        ));
    };

    let pending_on_proxy = if config.proxy_reward_token.is_some() {
        Some(MOCK_PROXY_REWARD_AMOUNT)
    } else {
        None
    };

    Ok(PendingTokenResponse {
        pending,
        pending_on_proxy,
    })
}
