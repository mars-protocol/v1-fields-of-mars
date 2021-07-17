use cosmwasm_std::{
    to_binary, Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, Extern, HumanAddr,
    MessageInfo, Querier, QueryRequest, StdError, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20QueryMsg, TokenInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

//----------------------------------------------------------------------------------------
// Asset
//----------------------------------------------------------------------------------------

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

    pub fn transfer_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
    ) -> StdResult<CosmosMsg> {
        self.info.transfer_message(from, to, self.amount)
    }

    pub fn transfer_from_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
    ) -> StdResult<CosmosMsg> {
        self.info.transfer_message(from, to, self.amount)
    }

    pub fn assert_sent_fund(&self, message: &MessageInfo) -> StdResult<()> {
        self.info.assert_send_fund(message, self.amount)
    }

    pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
        self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Self> {
        Ok(Asset {
            info: self.info,
            amount: self.info.deduct_tax(deps, self.amount)?,
        })
    }

    pub fn add_tax<S: Storage, A: Api, Q: Querier>(
        self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Self> {
        Ok(Asset {
            info: self.info,
            amount: self.info.add_tax(deps, self.amount)?,
        })
    }
}

//----------------------------------------------------------------------------------------
// AssetInfo
//----------------------------------------------------------------------------------------

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
    pub fn add_amount(self, amount: Uint128) -> Asset {
        Asset {
            info: self,
            amount,
        }
    }

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

    pub fn transfer_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
        amount: Uint128,
    ) -> StdResult<CosmosMsg> {
        match &self {
            AssetInfo::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from(to),
                    amount,
                })?,
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(from),
                to_address: HumanAddr::from(to),
                amount: vec![Coin {
                    denom: String::from(denom),
                    amount,
                }],
            })),
        }
    }

    pub fn transfer_from_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
        amount: Uint128,
    ) -> StdResult<CosmosMsg> {
        match &self {
            AssetInfo::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::TransferFrom {
                    owner: HumanAddr::from(from),
                    recipient: HumanAddr::from(to),
                    amount,
                })?,
            })),
            AssetInfo::NativeToken {
                ..
            } => Err(StdError::generic_err(
                "`TransferFrom` does not apply to native tokens",
            )),
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

    // @notice Verify whether specified amount of fund is sent along with a message
    pub fn assert_sent_fund(
        &self,
        message: &MessageInfo,
        amount: Uint128,
    ) -> StdResult<()> {
        if let AssetInfo::NativeToken {
            denom,
        } = &self.info {
            match message.sent_funds.iter().find(|fund| fund.denom == denom) {
                Some(fund) => {
                    if fund.amount == asset.amount {
                        Ok(())
                    } else {
                        Err(StdError::generic_err("sent fund mismatch"))
                    }
                }
                None => Err(StdError::generic_err("sent fund mismatch")),
            }
        } else {
            Ok(())
        }
    }

    /// Modified from
    /// https://github.com/terraswap/terraswap/blob/master/packages/terraswap/src/asset.rs#L58
    pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
        self,
        deps: &Extern<S, A, Q>,
        amount: Uint128,
    ) -> StdResult<Uint128> {
        match &self {
            AssetInfo::Token {
                ..
            } => Ok(Uint128::zero()),
            AssetInfo::NativeToken {
                denom,
            } => {
                let tax = if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(&deps.querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;
                    std::cmp::min(
                        (amount
                            - amount.multiply_ratio(
                                DECIMAL_FRACTION,
                                DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                            ))?,
                        tax_cap,
                    )
                };
                amount - tax
            }
        }
    }

    pub fn add_tax<S: Storage, A: Api, Q: Querier>(
        self,
        deps: &Extern<S, A, Q>,
        amount: Uint128,
    ) -> StdResult<Uint128> {
        match &self {
            AssetInfo::Token {
                ..
            } => Ok(amount),
            AssetInfo::NativeToken {
                denom,
            } => {
                let tax = if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(&deps.querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;
                    std::cmp::min(amount * tax_rate, tax_cap)
                };
                Ok(amount + tax)
            }
        }
    }
}

//----------------------------------------------------------------------------------------
// Raw Types
//----------------------------------------------------------------------------------------

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
