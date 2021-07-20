use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, Coin, Env, Extern, HandleResponse, HumanAddr,
    InitResponse, MigrateResponse, Querier, StdError, StdResult, Storage, Uint128,
};
use terra_cosmwasm::TerraQuerier;

use field_of_mars::red_bank::{
    DebtInfo, DebtResponse, HandleMsg, MockInitMsg, MockMigrateMsg, QueryMsg,
    RedBankAsset as Asset,
};

use crate::state::{read_config, read_position, write_config, write_position, Config};

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MockInitMsg,
) -> StdResult<InitResponse> {
    write_config(
        &mut deps.storage,
        &Config {
            mock_interest_rate: msg
                .mock_interest_rate
                .unwrap_or_else(|| Decimal256::one()),
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
        HandleMsg::Borrow {
            asset,
            amount,
        } => match asset {
            Asset::Native {
                denom,
            } => {
                if denom == "uluna" || denom == "uusd" {
                    borrow(deps, env, &denom[..], amount)
                } else {
                    Err(StdError::generic_err("unimplemented"))
                }
            }
            _ => Err(StdError::generic_err("unimplemented")),
        },
        HandleMsg::RepayNative {
            denom,
        } => {
            if denom == "uluna" || denom == "uusd" {
                repay(
                    deps,
                    env.clone(),
                    &denom[..],
                    get_denom_amount_from_coins(&env.message.sent_funds, &denom[..]),
                )
            } else {
                Err(StdError::generic_err("unimplemented"))
            }
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
    }
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: MockMigrateMsg,
) -> StdResult<MigrateResponse> {
    write_config(
        &mut deps.storage,
        &Config {
            mock_interest_rate: msg.mock_interest_rate.unwrap(),
        },
    )?;
    Ok(MigrateResponse::default())
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

fn borrow<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    denom: &str,
    amount: Uint256,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&env.message.sender)?;
    let mut position = read_position(&deps.storage, &user_raw, denom).unwrap_or_default();
    position.borrowed_amount += amount;
    write_position(&mut deps.storage, &user_raw, denom, &position)?;

    Ok(HandleResponse {
        messages: vec![BankMsg::Send {
            from_address: env.contract.address,
            to_address: env.message.sender.clone(),
            amount: vec![Coin {
                denom: denom.to_string(),
                amount: deduct_tax(&deps, amount.into(), denom)?,
            }],
        }
        .into()],
        log: vec![
            log("user", env.message.sender),
            log("denom", denom),
            log("amount", amount),
            log("borrowed_amount", position.borrowed_amount),
        ],
        data: None,
    })
}

fn repay<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    denom: &str,
    amount: Uint256,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&env.message.sender)?;
    let config = read_config(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw, denom).unwrap_or_default();

    let scaled_amount = amount / config.mock_interest_rate;
    position.borrowed_amount = position.borrowed_amount - scaled_amount;
    write_position(&mut deps.storage, &user_raw, denom, &position)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("user", env.message.sender),
            log("denom", denom),
            log("amount", amount),
            log("scaled_amount", scaled_amount),
            log("borrowed_amount", position.borrowed_amount),
        ],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

fn query_debt<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: HumanAddr,
) -> StdResult<DebtResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;

    let denoms = vec!["uluna", "uusd"];
    let debts = denoms
        .iter()
        .map(|denom| DebtInfo {
            denom: denom.to_string(),
            amount: read_position(&deps.storage, &user_raw, denom)
                .unwrap_or_default()
                .borrowed_amount
                * config.mock_interest_rate,
        })
        .collect();

    Ok(DebtResponse {
        debts,
    })
}

//----------------------------------------------------------------------------------------
// Helper Functions
//----------------------------------------------------------------------------------------

fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

fn deduct_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    amount: Uint128,
    denom: &str,
) -> StdResult<Uint128> {
    let tax = if denom == "uluna" {
        Ok(Uint128::zero())
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate = terra_querier.query_tax_rate()?.rate;
        let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;

        Ok(std::cmp::min(
            (amount
                - amount.multiply_ratio(
                    DECIMAL_FRACTION,
                    DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                ))?,
            tax_cap,
        ))
    };

    amount - tax?
}
