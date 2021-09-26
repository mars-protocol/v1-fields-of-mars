use cosmwasm_std::{
    entry_point, to_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, QueryRequest,
    Response, StdResult, Uint128, WasmQuery,
};

use fields_of_mars::asset::{Asset, AssetInfo};
use fields_of_mars::oracle::mock_msg::{ExecuteMsg, InstantiateMsg};
use fields_of_mars::oracle::msg::{AssetPriceResponse, QueryMsg};
use fields_of_mars::pool::msg::{QueryMsg as AstroportQueryMsg, SimulationResponse};

use crate::state::{Config, CONFIG};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        pair_address: deps.api.addr_validate(&msg.pair_address)?,
        token_address: deps.api.addr_validate(&msg.token_address)?,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: ExecuteMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AssetPriceByReference { .. } => to_binary(&query_asset_price(deps, env)?),
    }
}

fn query_asset_price(deps: Deps, env: Env) -> StdResult<AssetPriceResponse> {
    let config = CONFIG.load(deps.storage)?;

    let response: SimulationResponse =
        deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: config.pair_address.to_string(),
            msg: to_binary(&AstroportQueryMsg::Simulation {
                offer_asset: Asset {
                    info: AssetInfo::Token {
                        contract_addr: config.token_address,
                    },
                    amount: Uint128::new(1000000),
                },
            })?,
        }))?;

    let price = Decimal::from_ratio(
        response.return_amount + response.commission_amount,
        1000000u128,
    );

    Ok(AssetPriceResponse {
        price,
        last_updated: env.block.time.seconds(),
    })
}
