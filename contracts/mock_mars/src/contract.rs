use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, Coin, Env, Extern, HandleResponse, HumanAddr,
    InitResponse, Querier, StdError, StdResult, Storage,
};
use mars::liquidity_pool::{Asset, DebtInfo, DebtResponse, HandleMsg, InitMsg, QueryMsg};

use crate::{
    helpers::{deduct_tax, get_denom_amount_from_coins},
    state::{read_user, write_user, User},
};

//----------------------------------------------------------------------------------------
// ENTRY POINTS
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: InitMsg,
) -> StdResult<InitResponse> {
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Borrow {
            asset,
            amount,
        } => {
            match asset {
                Asset::Native {
                    denom,
                } => {
                    if denom != "uusd" {
                        return Err(StdError::generic_err("unimplemented"));
                    }
                }
                _ => {
                    return Err(StdError::generic_err("unimplemented"));
                }
            }
            borrow(deps, env, amount)
        }
        HandleMsg::RepayNative {
            denom,
        } => {
            if denom != "uusd" {
                return Err(StdError::generic_err("unimplemented"));
            }

            repay(
                deps,
                env.clone(),
                env.message.sender,
                get_denom_amount_from_coins(&env.message.sent_funds, "uusd"),
            )
        }
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Debt {
            address,
        } => to_binary(&query_debt(deps, address)?),
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

//----------------------------------------------------------------------------------------
// HANDLE FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn borrow<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    borrow_amount: Uint256,
) -> StdResult<HandleResponse> {
    let account_raw = deps.api.canonical_address(&env.message.sender)?;
    let borrowed_amount = read_user(&deps.storage, &account_raw)?.borrowed_amount;

    write_user(
        &mut deps.storage,
        &account_raw,
        &User {
            deposited_amount: Uint256::zero(),
            borrowed_amount: borrowed_amount + borrow_amount,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![BankMsg::Send {
            from_address: env.contract.address,
            to_address: env.message.sender,
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: deduct_tax(&deps, borrow_amount.into(), "uusd")?,
            }],
        }
        .into()],
        log: vec![log("borrowed_asset", "uusd"), log("borrowed_amount", borrowed_amount)],
        data: None,
    })
}

pub fn repay<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    repayer_address: HumanAddr,
    repay_amount: Uint256,
) -> StdResult<HandleResponse> {
    let account_raw = deps.api.canonical_address(&env.message.sender)?;
    let borrowed_amount = read_user(&deps.storage, &account_raw)?.borrowed_amount;

    write_user(
        &mut deps.storage,
        &account_raw,
        &User {
            deposited_amount: Uint256::zero(),
            borrowed_amount: borrowed_amount - repay_amount,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("repayed_address", repayer_address),
            log("repay_amount", repay_amount),
        ],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// QUERY FUNCTIONS
//----------------------------------------------------------------------------------------

fn query_debt<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    address: HumanAddr,
) -> StdResult<DebtResponse> {
    let user = read_user(&deps.storage, &deps.api.canonical_address(&address)?)?;
    Ok(DebtResponse {
        debts: vec![DebtInfo {
            denom: String::from("uusd"),
            amount: user.borrowed_amount,
        }],
    })
}
