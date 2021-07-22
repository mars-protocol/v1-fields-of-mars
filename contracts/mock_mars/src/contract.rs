#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    attr, to_binary, BankMsg, Binary, Coin, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, SubMsg, Uint128,
};
use terra_cosmwasm::TerraQuerier;

use field_of_mars::red_bank::{
    DebtInfo, DebtResponse, ExecuteMsg, MockInstantiateMsg, MockMigrateMsg, QueryMsg,
    RedBankAsset as Asset,
};

use crate::state::{Config, CONFIG, POSITION};

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

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
    CONFIG.save(deps.storage, &Config::new(msg))?;
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
        ExecuteMsg::Borrow {
            asset,
            amount,
        } => match asset {
            Asset::Native {
                denom,
            } => {
                if denom == "uluna" || denom == "uusd" {
                    borrow(deps, env, info, &denom[..], amount)
                } else {
                    Err(StdError::generic_err("unimplemented"))
                }
            }
            _ => Err(StdError::generic_err("unimplemented")),
        },
        ExecuteMsg::RepayNative {
            denom,
        } => {
            if denom == "uluna" || denom == "uusd" {
                let amount = get_denom_amount_from_coins(&info.funds, &denom[..]);
                repay(deps, env, info, &denom[..], amount)
            } else {
                Err(StdError::generic_err("unimplemented"))
            }
        }
        _ => Err(StdError::generic_err("unimplemented")),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Debt {
            address,
        } => to_binary(&query_debt(deps, env, address)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MockMigrateMsg) -> StdResult<Response> {
    CONFIG.save(deps.storage, &Config::new(msg))?;
    Ok(Response::default())
}

//----------------------------------------------------------------------------------------
// Execute Functions
//----------------------------------------------------------------------------------------

fn borrow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    denom: &str,
    amount: Uint256,
) -> StdResult<Response> {
    let mut position =
        POSITION.load(deps.storage, (&info.sender, denom)).unwrap_or_default();

    position.borrowed_amount += amount;
    POSITION.save(deps.storage, (&info.sender, denom), &position)?;

    Ok(Response {
        messages: vec![SubMsg::new(BankMsg::Send {
            to_address: String::from(&info.sender),
            amount: vec![Coin {
                denom: denom.to_string(),
                amount: deduct_tax(deps.as_ref(), denom, amount.into())?,
            }],
        })],
        attributes: vec![
            attr("user", info.sender),
            attr("denom", denom),
            attr("amount", amount),
            attr("borrowed_amount", position.borrowed_amount),
        ],
        events: vec![],
        data: None,
    })
}

fn repay(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    denom: &str,
    amount: Uint256,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, (&info.sender, denom))?;

    let scaled_amount = amount / config.mock_interest_rate;
    position.borrowed_amount = position.borrowed_amount - scaled_amount;
    POSITION.save(deps.storage, (&info.sender, denom), &position)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![
            attr("user", info.sender),
            attr("denom", denom),
            attr("amount", amount),
            attr("scaled_amount", scaled_amount),
            attr("borrowed_amount", position.borrowed_amount),
        ],
        events: vec![],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

fn query_debt(deps: Deps, _env: Env, user: String) -> StdResult<DebtResponse> {
    let addr = deps.api.addr_validate(&user)?;
    let config = CONFIG.load(deps.storage)?;

    let compute_debt_amount = |denom: &str| {
        let position = POSITION.load(deps.storage, (&addr, denom)).unwrap_or_default();
        position.borrowed_amount * config.mock_interest_rate
    };

    let debts = ["uluna", "uusd"]
        .iter()
        .map(|denom| DebtInfo {
            denom: denom.to_string(),
            amount: compute_debt_amount(denom),
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
        .unwrap_or(Uint256::zero())
}

fn deduct_tax(deps: Deps, denom: &str, amount: Uint128) -> StdResult<Uint128> {
    let tax = if denom == "uluna" {
        Uint128::zero()
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate = terra_querier.query_tax_rate()?.rate;
        let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;
        std::cmp::min(
            amount.checked_sub(amount.multiply_ratio(
                DECIMAL_FRACTION,
                DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
            ))?,
            tax_cap,
        )
    };
    Ok(amount - tax)
}
