use std::str;

use cosmwasm_std::{
    entry_point, to_binary, to_vec, Binary, Deps, DepsMut, Empty, Env, MessageInfo, QueryRequest,
    Response, StdError, StdResult, WasmQuery,
};

use cw_asset::Asset;

use mars_core::asset::Asset as MarsAsset;
use mars_core::math::decimal::Decimal as MarsDecimal;
use mars_core::oracle::msg::{ExecuteMsg, QueryMsg};
use mars_core::oracle::{PriceSourceChecked, PriceSourceUnchecked};

use astroport::pair::{QueryMsg as AstroportQueryMsg, SimulationResponse};

use crate::state::PRICE_SOURCE;

static PROBE_AMOUNT: u128 = 1000000;

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
        ExecuteMsg::SetAsset {
            asset,
            price_source,
        } => execute_set_asset(deps, env, info, asset, price_source),

        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented execute: {}", String::from_utf8(to_vec(&msg)?)?)
        )),
    }
}

fn execute_set_asset(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    asset: MarsAsset,
    price_source: PriceSourceUnchecked,
) -> StdResult<Response> {
    let asset_reference = asset.get_reference();

    let price_source_checked = match price_source {
        PriceSourceUnchecked::Fixed {
            price,
        } => PriceSourceChecked::Fixed {
            price,
        },
        PriceSourceUnchecked::AstroportSpot {
            pair_address,
        } => PriceSourceChecked::AstroportSpot {
            pair_address: deps.api.addr_validate(&pair_address)?,
        },

        ps => {
            return Err(StdError::generic_err(
                format!("[mock] unimplemented price source: {}", String::from_utf8(to_vec(&ps)?)?)
            ));
        }
    };

    PRICE_SOURCE.save(deps.storage, &asset_reference, &price_source_checked)?;

    Ok(Response::default())
}

// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AssetPrice {
            asset,
        } => to_binary(&query_asset_price(deps, env, &asset.get_reference())?),
        QueryMsg::AssetPriceByReference {
            asset_reference,
        } => to_binary(&query_asset_price(deps, env, &asset_reference)?),

        _ => Err(StdError::generic_err(
            format!("[mock] unimplemented query: {}", String::from_utf8(to_vec(&msg)?)?)
        )),
    }
}

fn query_asset_price(deps: Deps, _env: Env, asset_reference: &[u8]) -> StdResult<MarsDecimal> {
    let price_source = PRICE_SOURCE.load(deps.storage, asset_reference)?;

    let price = match price_source {
        PriceSourceChecked::Fixed {
            price,
        } => price,

        PriceSourceChecked::AstroportSpot {
            pair_address,
        } => {
            // for this mock contract, we assume asset is a CW20, in which case `asset_reference`
            // is the token contract address
            let asset_address_str = str::from_utf8(asset_reference)?;
            let asset_address = deps.api.addr_validate(asset_address_str)?;
            let offer_asset = Asset::cw20(asset_address, PROBE_AMOUNT);

            let response: SimulationResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: pair_address.to_string(),
                    msg: to_binary(&AstroportQueryMsg::Simulation {
                        offer_asset: offer_asset.into(),
                    })?,
                }))?;

            MarsDecimal::from_ratio(
                response.return_amount + response.commission_amount,
                PROBE_AMOUNT,
            )
        }

        ps => {
            return Err(StdError::generic_err(
                format!("[mock] unimplemented price source: {}", String::from_utf8(to_vec(&ps)?)?)
            ))
        }
    };

    Ok(price)
}
