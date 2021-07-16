use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, Extern, HumanAddr, Querier, QueryRequest, StdResult,
    Storage, WasmQuery,
};
use cw20::{Cw20QueryMsg, TokenInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    Cw20 {
        contract_addr: HumanAddr,
    },
    Native {
        denom: String,
    },
}

impl Asset {
    /// @notice Convert `Asset` to `AssetRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetRaw> {
        match self {
            Asset::Cw20 {
                contract_addr,
            } => Ok(AssetRaw::Cw20 {
                contract_addr: deps.api.canonical_address(&contract_addr)?,
            }),
            Asset::Native {
                denom,
            } => Ok(AssetRaw::Native {
                denom: String::from(denom),
            }),
        }
    }

    /// @notice Query the denomination of the asset
    /// @dev If the asset is a CW20, query `TokenInfo` and returm `symbol`
    pub fn query_denom<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<String> {
        match self {
            Asset::Cw20 {
                contract_addr,
            } => {
                let response: TokenInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: HumanAddr::from(contract_addr),
                        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                Ok(response.symbol)
            }
            Asset::Native {
                denom,
            } => Ok(String::from(denom)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetRaw {
    Cw20 {
        contract_addr: CanonicalAddr,
    },
    Native {
        denom: String,
    },
}

impl AssetRaw {
    /// @notice Convert `AssetRaw` to `Asset`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Asset> {
        match self {
            AssetRaw::Cw20 {
                contract_addr,
            } => Ok(Asset::Cw20 {
                contract_addr: deps.api.human_address(&contract_addr)?,
            }),
            AssetRaw::Native {
                denom,
            } => Ok(Asset::Native {
                denom: String::from(denom),
            }),
        }
    }
}
