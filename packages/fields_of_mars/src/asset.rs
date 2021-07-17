use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, Extern, HumanAddr, Querier, QueryRequest, StdResult,
    Storage, Uint128, WasmQuery,
};
use cw20::{Cw20QueryMsg, TokenInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl Asset {
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetRaw> {
        Ok(AssetRaw {
            info: self.info.to_raw(deps)?,
            amount: self.amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo {
    Token {
        contract_addr: HumanAddr,
    },
    NativeToken {
        denom: String,
    },
}

impl AssetInfo {
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetInfoRaw> {
        match &self {
            AssetInfo::Token {
                contract_addr,
            } => Ok(AssetInfoRaw::Token {
                contract_addr: deps.api.canonical_address(&contract_addr)?,
            }),
            AssetInfo::NativeToken {
                denom,
            } => Ok(AssetInfoRaw::NativeToken {
                denom: String::from(denom),
            }),
        }
    }

    /// @notice Compare if two `AssetInfo` objects are the same
    pub fn equals(&self, info: &AssetInfo) -> bool {
        match &self {
            AssetInfo::Token {
                contract_addr,
            } => {
                let self_addr = contract_addr;
                match info {
                    AssetInfo::Token {
                        contract_addr,
                    } => self_addr == contract_addr,
                    AssetInfo::NativeToken {
                        ..
                    } => false,
                }
            }
            AssetInfo::NativeToken {
                denom,
            } => {
                let self_denom = denom;
                match info {
                    AssetInfo::NativeToken {
                        denom,
                    } => self_denom == denom,
                    AssetInfo::Token {
                        ..
                    } => false,
                }
            }
        }
    }

    /// @notice Query the denomination of the asset
    /// @dev If the asset is a CW20, query `TokenInfo` and returm `symbol`
    pub fn query_denom<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<String> {
        match self {
            AssetInfo::Token {
                contract_addr,
            } => {
                let response: TokenInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: HumanAddr::from(contract_addr),
                        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                Ok(response.symbol)
            }
            AssetInfo::NativeToken {
                denom,
            } => Ok(String::from(denom)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetRaw {
    pub info: AssetInfoRaw,
    pub amount: Uint128,
}

impl AssetRaw {
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Asset> {
        Ok(Asset {
            info: self.info.to_normal(deps)?,
            amount: self.amount,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfoRaw {
    Token {
        contract_addr: CanonicalAddr,
    },
    NativeToken {
        denom: String,
    },
}

impl AssetInfoRaw {
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetInfo> {
        match &self {
            AssetInfoRaw::Token {
                contract_addr,
            } => Ok(AssetInfo::Token {
                contract_addr: deps.api.human_address(&contract_addr)?,
            }),
            AssetInfoRaw::NativeToken {
                denom,
            } => Ok(AssetInfo::NativeToken {
                denom: String::from(denom),
            }),
        }
    }
}
