use cosmwasm_std::{
    to_binary, Api, BankMsg, CanonicalAddr, Coin, CosmosMsg, Extern, HumanAddr,
    MessageInfo, Querier, QueryRequest, StdError, StdResult, Storage, Uint128, WasmMsg,
    WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20QueryMsg, TokenInfoResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, ops};
use terra_cosmwasm::TerraQuerier;

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

//----------------------------------------------------------------------------------------
// Asset
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl Into<Uint128> for Asset {
    fn into(self) -> Uint128 {
        self.amount
    }
}

impl Ord for Asset {
    fn cmp(&self, other: &Self) -> Ordering {
        self.assert_matched_info(&other);
        self.amount.cmp(&other.amount)
    }
}

impl PartialOrd for Asset {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl ops::Add for Asset {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.assert_matched_info(&other);
        Asset {
            info: self.info,
            amount: self.amount + other.amount,
        }
    }
}

impl ops::AddAssign for Asset {
    fn add_assign(&mut self, other: Self) {
        self.assert_matched_info(&other);
        self.amount += other.amount;
    }
}

impl ops::Sub for Asset {
    type Output = StdResult<Self>;

    fn sub(self, other: Self) -> StdResult<Self> {
        self.assert_matched_info(&other);
        Ok(Asset {
            info: self.info,
            amount: (self.amount - other.amount)?,
        })
    }
}

impl Asset {
    /// @notice Arithmatic operation `multiply_into` (see `cosmwasm_std::Uint128` docs)
    /// @dev `nom` and `denom` can be assets of different `info`
    pub fn multiply_ratio(&self, nom: Uint128, denom: Uint128) -> Self {
        Asset {
            info: self.info.clone(),
            amount: self.amount.multiply_ratio(nom, denom),
        }
    }

    /// @notice Check if the amount is zero
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// @notice Convert `Asset` to `AssetRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetRaw> {
        Ok(AssetRaw {
            info: self.info.to_raw(deps)?,
            amount: self.amount,
        })
    }

    /// @notice Assert two assets have the same `info`; panic if not
    pub fn assert_matched_info(&self, other: &Self) {
        self.info.assert_matched_info(&other.info);
    }

    /// @notice Assert specified amount of fund is sent along with a message; panic if not
    pub fn assert_sent_fund(&self, message: &MessageInfo) {
        self.info.assert_sent_fund(&message, self.amount);
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::Transfer` message type
    pub fn transfer_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
    ) -> StdResult<CosmosMsg> {
        self.info.transfer_message(from, to, self.amount)
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::TransferFrom` message type
    /// @dev Must have allowance
    pub fn transfer_from_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
    ) -> StdResult<CosmosMsg> {
        self.info.transfer_message(from, to, self.amount)
    }

    /// @notice Query the denomination of the asset
    /// @dev If the asset is a CW20, query `TokenInfo` and returm `symbol`
    pub fn query_denom<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<String> {
        self.info.query_denom(deps)
    }

    /// @notice Return an asset whose amount reflects the deliverable amount if the asset
    /// is to be transferred.
    /// @dev For example, if the asset is 1000 UST, and the tax for sending 1000 UST is
    /// 1 UST, then update amount to 1000 - 1 = 999.
    pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Self> {
        Ok(Asset {
            info: self.info.clone(),
            amount: self.info.deduct_tax(deps, self.amount)?,
        })
    }

    /// @notice Return an asset whose amount reflects the total amount needed to deliver
    /// the specified amount.
    /// @dev For example, if the asset is 1000 UST, and and 1 UST of tax is to be charged
    /// to deliver 1000 UST, then updatethe amount to 1000 + 1 = 1001.
    pub fn add_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Self> {
        Ok(Asset {
            info: self.info.clone(),
            amount: self.info.add_tax(deps, self.amount)?,
        })
    }
}

//----------------------------------------------------------------------------------------
// AssetInfo
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
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
    /// @notice Convert `AssetInfo` to `Asset` by adding an amount
    pub fn add_amount(&self, amount: Uint128) -> Asset {
        Asset {
            info: self.clone(),
            amount,
        }
    }

    /// @notice Convert `AssetInfo` to `AssetInfoRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<AssetInfoRaw> {
        match &self {
            Self::Token {
                contract_addr,
            } => Ok(AssetInfoRaw::Token {
                contract_addr: deps.api.canonical_address(&contract_addr)?,
            }),
            Self::NativeToken {
                denom,
            } => Ok(AssetInfoRaw::NativeToken {
                denom: String::from(denom),
            }),
        }
    }

    /// @notice Assert two asset types are the same; panic if not
    pub fn assert_matched_info(&self, other: &Self) {
        if !&self.equals(&other) {
            panic!("asset info mismatch!")
        }
    }

    /// @notice Assert specified amount of fund is sent along with a message; panic if not
    pub fn assert_sent_fund(&self, message: &MessageInfo, amount: Uint128) {
        if let AssetInfo::NativeToken {
            denom,
        } = &self
        {
            match message.sent_funds.iter().find(|fund| &fund.denom == denom) {
                Some(fund) => {
                    if fund.amount != amount {
                        panic!("sent fund mismatch!");
                    }
                }
                None => {
                    panic!("sent fund mismatch!");
                }
            }
        }
    }

    /// @notice Compare if two `AssetInfo` objects are the same
    pub fn equals(&self, info: &AssetInfo) -> bool {
        match &self {
            Self::Token {
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
            Self::NativeToken {
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

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::Transfer` message type
    pub fn transfer_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
        amount: Uint128,
    ) -> StdResult<CosmosMsg> {
        match &self {
            Self::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: HumanAddr::from(to),
                    amount,
                })?,
            })),
            Self::NativeToken {
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

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::TransferFrom` message type
    /// @dev Must have allowance
    pub fn transfer_from_message(
        &self,
        from: &HumanAddr,
        to: &HumanAddr,
        amount: Uint128,
    ) -> StdResult<CosmosMsg> {
        match &self {
            Self::Token {
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
            Self::NativeToken {
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
        match &self {
            Self::Token {
                contract_addr,
            } => {
                let response: TokenInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: HumanAddr::from(contract_addr),
                        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                Ok(response.symbol)
            }
            Self::NativeToken {
                denom,
            } => Ok(String::from(denom)),
        }
    }

    /// @notice Update the asset amount to reflect the deliverable amount if the asset is
    /// to be transferred.
    /// @dev For example, if the asset is 1000 UST, and the tax for sending 1000 UST is
    /// 1 UST, then update amount to 1000 - 1 = 999.
    /// @dev Modified from
    /// https://github.com/terraswap/terraswap/blob/master/packages/terraswap/src/asset.rs#L58
    pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        amount: Uint128,
    ) -> StdResult<Uint128> {
        match &self {
            Self::Token {
                ..
            } => Ok(Uint128::zero()),
            Self::NativeToken {
                denom,
            } => {
                let tax = if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(&deps.querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(String::from(denom))?.cap;
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

    /// @notice Update the asset amount to reflect the total amount needed to deliver the
    /// specified amount.
    /// @dev For example, if the asset is 1000 UST, and and 1 UST of tax is to be charged
    /// to deliver 1000 UST, then updatethe amount to 1000 + 1 = 1001.
    pub fn add_tax<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        amount: Uint128,
    ) -> StdResult<Uint128> {
        match &self {
            Self::Token {
                ..
            } => Ok(amount),
            Self::NativeToken {
                denom,
            } => {
                let tax = if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(&deps.querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(String::from(denom))?.cap;
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
            Self::Token {
                contract_addr,
            } => Ok(AssetInfo::Token {
                contract_addr: deps.api.human_address(&contract_addr)?,
            }),
            Self::NativeToken {
                denom,
            } => Ok(AssetInfo::NativeToken {
                denom: String::from(denom),
            }),
        }
    }
}
