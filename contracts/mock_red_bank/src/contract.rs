use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;

use fields_of_mars::adapters::Asset;

use mars::asset::Asset as MarsAsset;
use red_bank::msg::{DebtInfo, DebtResponse, QueryMsg, ReceiveMsg};

use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::state::DEBT_AMOUNT;

// This mock contract currently only supports borrowing uluna and uusd
// Borrowing of CW20 is not needed for testing purpose, so not implemented
static SUPPORTED_ASSETS: [&str; 2] = ["uluna", "uusd"];

// INIT

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {
    Ok(Response::default()) // do nothing
}

// EXECUTE

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::Borrow { asset, amount } => execute_borrow(deps, env, info, asset, amount),
        ExecuteMsg::RepayNative { denom } => {
            let repayer_addr = info.sender.clone();
            let repay_amount = helpers::get_denom_amount_from_coins(&info.funds, &denom);
            execute_repay(deps, env, info, repayer_addr, &denom, repay_amount)
        }
        ExecuteMsg::SetUserDebt {
            user_address,
            denom,
            amount,
        } => {
            let user_addr = deps.api.addr_validate(&user_address)?;
            execute_set_debt(deps, env, info, user_addr, &denom, amount)
        }
    }
}

pub fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    match from_binary(&cw20_msg.msg)? {
        ReceiveMsg::RepayCw20 {} => {
            let repayer_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            let denom = info.sender.to_string();
            execute_repay(deps, env, info, repayer_addr, &denom, cw20_msg.amount)
        }

        _ => Err(StdError::generic_err("Unimplemented")),
    }
}

fn execute_borrow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset: MarsAsset,
    amount: Uint128,
) -> StdResult<Response> {
    let denom = match &asset {
        MarsAsset::Cw20 { contract_addr } => &contract_addr[..],
        MarsAsset::Native { denom } => &denom[..],
    };

    let debt_amount = helpers::load_debt_amount(deps.storage, &info.sender, denom);

    DEBT_AMOUNT.save(deps.storage, (&info.sender, denom), &(debt_amount + amount))?;

    let outbound_asset = match &asset {
        MarsAsset::Cw20 { contract_addr } => {
            Asset::cw20(&deps.api.addr_validate(contract_addr)?, amount)
        }
        MarsAsset::Native { denom } => Asset::native(denom, amount),
    };

    Ok(Response::new().add_message(outbound_asset.transfer_msg(&info.sender)?))
}

fn execute_repay(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    denom: &str,
    amount: Uint128,
) -> StdResult<Response> {
    let debt_amount = helpers::load_debt_amount(deps.storage, &user_addr, denom);

    DEBT_AMOUNT.save(deps.storage, (&user_addr, denom), &(debt_amount - amount))?;

    Ok(Response::default())
}

fn execute_set_debt(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    user_addr: Addr,
    denom: &str,
    amount: Uint128,
) -> StdResult<Response> {
    DEBT_AMOUNT.save(deps.storage, (&user_addr, denom), &amount)?;

    Ok(Response::default())
}

// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::UserDebt { user_address } => to_binary(&query_debt(deps, env, user_address)?),

        _ => Err(StdError::generic_err("Unimplemented")),
    }
}

fn query_debt(deps: Deps, _env: Env, user_address: String) -> StdResult<DebtResponse> {
    let user_addr = deps.api.addr_validate(&user_address)?;

    let debts = SUPPORTED_ASSETS
        .iter()
        .map(|denom| DebtInfo {
            denom: denom.to_string(),
            amount_scaled: helpers::load_debt_amount(deps.storage, &user_addr, denom),
        })
        .collect();

    Ok(DebtResponse { debts })
}

// HELPERS

pub mod helpers {
    use cosmwasm_std::{Addr, Coin, Storage, Uint128};

    use crate::state::DEBT_AMOUNT;

    pub fn load_debt_amount(storage: &dyn Storage, user: &Addr, denom: &str) -> Uint128 {
        DEBT_AMOUNT
            .load(storage, (user, denom))
            .unwrap_or_else(|_| Uint128::zero())
    }

    pub fn get_denom_amount_from_coins(funds: &[Coin], denom: &str) -> Uint128 {
        funds
            .iter()
            .find(|coin| coin.denom == denom)
            .map(|coin| coin.amount)
            .unwrap_or_else(Uint128::zero)
    }
}
