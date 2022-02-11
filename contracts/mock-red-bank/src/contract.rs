use cosmwasm_std::{
    entry_point, from_binary, to_binary, to_vec, Addr, Binary, Deps, DepsMut, Empty, Env, 
    MessageInfo, Response, StdError, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;

use cw_asset::Asset;

use mars_core::asset::{Asset as MarsAsset, AssetType as MarsAssetType};
use mars_core::red_bank::msg::{QueryMsg, ReceiveMsg};
use mars_core::red_bank::UserAssetDebtResponse;

use crate::msg::ExecuteMsg;
use crate::state::DEBT_AMOUNT;

// INIT

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Empty,
) -> StdResult<Response> {
    Ok(Response::default()) // do nothing
}

// EXECUTE

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::Borrow {
            asset,
            amount,
        } => execute_borrow(deps, env, info, asset, amount),
        ExecuteMsg::RepayNative {
            denom,
        } => {
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
        ReceiveMsg::RepayCw20 { .. } => {
            let repayer_addr = deps.api.addr_validate(&cw20_msg.sender)?;
            let denom = info.sender.to_string();
            execute_repay(deps, env, info, repayer_addr, &denom, cw20_msg.amount)
        }
        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented receiver: {}", String::from_utf8(to_vec(&cw20_msg)?)?)
        )),
    }
}

fn execute_borrow(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset: MarsAsset,
    amount: Uint128,
) -> StdResult<Response> {
    let denom = helpers::get_asset_denom(&deps.querier, &asset)?;
    let debt_amount = helpers::load_debt_amount(deps.storage, &info.sender, &denom);

    DEBT_AMOUNT.save(deps.storage, (&info.sender, &denom), &(debt_amount + amount))?;

    // NOTE: we will implement `Into<cw_asset::Asset>` for `MarsAsset` once mars-core has been open-
    // sourced, after which these lines can be simpified:
    //
    // ```rust
    // let outbound_asset = asset.into();
    // ```
    let outbound_asset = match &asset {
        MarsAsset::Cw20 {
            contract_addr,
        } => Asset::cw20(deps.api.addr_validate(contract_addr)?, amount),
        MarsAsset::Native {
            denom,
        } => Asset::native(denom, amount),
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
    let mut debt_amount = helpers::load_debt_amount(deps.storage, &user_addr, denom);

    // If the user pays more than what they owe, we simply reduce the debt amount to zero
    // The actual Red Bank contract refunds the excess payment amount. But this difference is ok for
    // testing purpose
    debt_amount = debt_amount.checked_sub(amount).unwrap_or_else(|_| Uint128::zero());

    DEBT_AMOUNT.save(deps.storage, (&user_addr, denom), &debt_amount)?;

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
        QueryMsg::UserAssetDebt {
            user_address,
            asset,
        } => to_binary(&query_debt(deps, env, user_address, asset)?),

        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented query: {}", String::from_utf8(to_vec(&msg)?)?)
        )),
    }
}

fn query_debt(
    deps: Deps,
    _env: Env,
    user_address: String,
    asset: MarsAsset,
) -> StdResult<UserAssetDebtResponse> {
    let user_addr = deps.api.addr_validate(&user_address)?;
    let denom = helpers::get_asset_denom(&deps.querier, &asset)?;
    let debt_amount = helpers::load_debt_amount(deps.storage, &user_addr, &denom);
    Ok(UserAssetDebtResponse {
        // only amount matters for our testing
        amount: debt_amount,
        // for other attributes we just fill in some random value
        denom: "".to_string(),
        asset_label: "".to_string(),
        asset_reference: vec![],
        asset_type: MarsAssetType::Native,
        amount_scaled: Uint128::zero(),
    })
}

// HELPERS

pub mod helpers {
    use super::*;
    use cosmwasm_std::{Coin, QuerierWrapper, QueryRequest, Storage, WasmQuery};
    use cw20::{Cw20QueryMsg, TokenInfoResponse};

    pub fn load_debt_amount(storage: &dyn Storage, user: &Addr, denom: &str) -> Uint128 {
        DEBT_AMOUNT.load(storage, (user, denom)).unwrap_or_else(|_| Uint128::zero())
    }

    pub fn get_denom_amount_from_coins(funds: &[Coin], denom: &str) -> Uint128 {
        funds
            .iter()
            .find(|coin| coin.denom == denom)
            .map(|coin| coin.amount)
            .unwrap_or_else(Uint128::zero)
    }

    pub fn get_asset_denom(querier: &QuerierWrapper, asset: &MarsAsset) -> StdResult<String> {
        let denom = match asset {
            MarsAsset::Native {
                denom,
            } => denom.clone(),
            MarsAsset::Cw20 {
                contract_addr,
            } => {
                let response: TokenInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.clone(),
                        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                response.symbol
            }
        };
        Ok(denom)
    }
}
