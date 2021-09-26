use cosmwasm_std::{
    entry_point, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest,
    Response, StdResult, Uint128, WasmQuery,
};

use fields_of_mars::asset::{AssetChecked, AssetInfoChecked, AssetInfoUnchecked};
use fields_of_mars::oracle::mock_msg::{
    ExecuteMsg, InstantiateMsg, PriceSourceChecked, PriceSourceUnchecked,
};
use fields_of_mars::oracle::msg::{AssetPriceResponse, QueryMsg};
use fields_of_mars::pool::msg::{QueryMsg as AstroportQueryMsg, SimulationResponse};

use crate::state::PRICE_SOURCE;

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
        ExecuteMsg::SetAsset {
            asset_info,
            price_source,
        } => execute_set_asset(deps, env, info, asset_info, price_source),
    }
}

fn execute_set_asset(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    asset_info: AssetInfoUnchecked,
    price_source: PriceSourceUnchecked,
) -> StdResult<Response> {
    let asset_reference = asset_info.check(deps.api)?.get_reference();

    let price_source_checked = match price_source {
        PriceSourceUnchecked::Fixed { price } => PriceSourceChecked::Fixed { price },
        PriceSourceUnchecked::AstroportSpot {
            pair_address,
            asset_address,
        } => PriceSourceChecked::AstroportSpot {
            pair_address: deps.api.addr_validate(&pair_address)?,
            asset_address: deps.api.addr_validate(&asset_address)?,
        },
    };

    PRICE_SOURCE.save(deps.storage, &asset_reference, &price_source_checked)?;

    Ok(Response::default())
}

// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AssetPriceByReference { asset_reference } => {
            to_binary(&query_asset_price(deps, env, &asset_reference)?)
        }
    }
}

fn query_asset_price(
    deps: Deps,
    env: Env,
    asset_reference: &[u8],
) -> StdResult<AssetPriceResponse> {
    let price_source = PRICE_SOURCE.load(deps.storage, asset_reference)?;

    let price = match price_source {
        PriceSourceChecked::Fixed { price } => price,

        PriceSourceChecked::AstroportSpot {
            pair_address,
            asset_address,
        } => {
            let offer_asset = AssetChecked {
                info: AssetInfoChecked::Token {
                    contract_addr: asset_address,
                },
                amount: Uint128::new(1000000),
            };

            let response: SimulationResponse =
                deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: pair_address.to_string(),
                    msg: to_binary(&AstroportQueryMsg::Simulation { offer_asset })?,
                }))?;

            Decimal::from_ratio(
                response.return_amount + response.commission_amount,
                1000000u128,
            )
        }
    };

    Ok(AssetPriceResponse {
        price,
        last_updated: env.block.time.seconds(),
    })
}
