use cosmwasm_std::{
    to_binary, Addr, BalanceResponse, BankMsg, BankQuery, Coin, MessageInfo,
    QuerierWrapper, QueryRequest, StdError, StdResult, SubMsg, Uint128, WasmMsg,
    WasmQuery,
};
use cw20::{
    BalanceResponse as Cw20BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg,
    TokenInfoResponse as Cw20TokenInfoResponse,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terra_cosmwasm::TerraQuerier;

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

//----------------------------------------------------------------------------------------
// Asset
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

impl Asset {
    /// @notice Assert two assets have the same `info`; panic if not
    pub fn assert_matched_info(&self, other: &Self) -> StdResult<()> {
        self.info.assert_matched_info(&other.info)
    }

    /// @notice Assert specified amount of fund is sent along with a message; panic if not
    pub fn assert_sent_fund(&self, message: &MessageInfo) -> StdResult<()> {
        self.info.assert_sent_fund(&message, self.amount)
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::Transfer` message type
    pub fn transfer_msg(&self, to: &Addr) -> StdResult<SubMsg> {
        self.info.transfer_msg(to, self.amount)
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::TransferFrom` message type
    /// @dev Must have allowance
    pub fn transfer_from_msg(&self, from: &Addr, to: &Addr) -> StdResult<SubMsg> {
        self.info.transfer_from_msg(from, to, self.amount)
    }

    /// @notice Query the denomination of the asset
    /// @dev If the asset is a CW20, query `TokenInfo` and returm `symbol`
    pub fn query_denom(&self, querier: &QuerierWrapper) -> StdResult<String> {
        self.info.query_denom(querier)
    }

    /// @notice Return an asset whose amount reflects the deliverable amount if the asset
    /// is to be transferred.
    /// @dev For example, if the asset is 1000 UST, and the tax for sending 1000 UST is
    /// 1 UST, then update amount to 1000 - 1 = 999.
    pub fn deduct_tax(&self, querier: &QuerierWrapper) -> StdResult<Self> {
        Ok(Asset {
            info: self.info.clone(),
            amount: self.info.deduct_tax(querier, self.amount)?,
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
        contract_addr: String, // user-provided values should be String instead of String
    },
    NativeToken {
        denom: String,
    },
}

impl AssetInfo {
    /// @notice Initialize an `Asset` object with zero amount
    pub fn zero(&self) -> Asset {
        Asset {
            info: self.clone(),
            amount: Uint128::zero(),
        }
    }

    /// @notice Assert two asset types are the same; panic if not
    pub fn assert_matched_info(&self, other: &Self) -> StdResult<()> {
        if self == other {
            Ok(())
        } else {
            Err(StdError::generic_err("asset info mismatch!"))
        }
    }

    /// @notice Assert specified amount of fund is sent along with a message; panic if not
    pub fn assert_sent_fund(
        &self,
        message: &MessageInfo,
        amount: Uint128,
    ) -> StdResult<()> {
        if let AssetInfo::NativeToken {
            denom,
        } = self
        {
            match message.funds.iter().find(|fund| &fund.denom == denom) {
                Some(fund) => {
                    if fund.amount != amount {
                        return Err(StdError::generic_err("sent fund mismatch!"));
                    }
                }
                None => {
                    if !amount.is_zero() {
                        return Err(StdError::generic_err("sent fund mismatch!"));
                    }
                }
            }
        }
        Ok(())
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20ExecuteMsg::Transfer` message type
    /// @dev Note: `amount` must have tax deducted before passing into this function!
    pub fn transfer_msg(&self, to: &Addr, amount: Uint128) -> StdResult<SubMsg> {
        match self {
            Self::Token {
                contract_addr,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from(to),
                    amount,
                })?,
                funds: vec![],
            })),
            Self::NativeToken {
                denom,
            } => Ok(SubMsg::new(BankMsg::Send {
                to_address: String::from(to),
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount,
                }],
            })),
        }
    }

    /// @notice Generate the message for transferring asset of a specific amount from one
    /// account to another using the `Cw20HandleMsg::TransferFrom` message type
    /// @dev Must have allowance
    pub fn transfer_from_msg(
        &self,
        from: &Addr,
        to: &Addr,
        amount: Uint128,
    ) -> StdResult<SubMsg> {
        match self {
            Self::Token {
                contract_addr,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from(from),
                    recipient: String::from(to),
                    amount,
                })?,
                funds: vec![],
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
    pub fn query_denom(&self, querier: &QuerierWrapper) -> StdResult<String> {
        match self {
            Self::Token {
                contract_addr,
            } => {
                let response: Cw20TokenInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.clone(),
                        msg: to_binary(&Cw20QueryMsg::TokenInfo {})?,
                    }))?;
                Ok(response.symbol)
            }
            Self::NativeToken {
                denom,
            } => Ok(denom.clone()),
        }
    }

    /// @notice Query an account's balance of the specified asset
    pub fn query_balance(
        &self,
        querier: &QuerierWrapper,
        account: &String,
    ) -> StdResult<Uint128> {
        match self {
            Self::Token {
                contract_addr,
            } => {
                let response: Cw20BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.clone(),
                        msg: to_binary(&Cw20QueryMsg::Balance {
                            address: account.clone(),
                        })?,
                    }))?;
                Ok(response.balance)
            }
            Self::NativeToken {
                denom,
            } => {
                let response: BalanceResponse =
                    querier.query(&QueryRequest::Bank(BankQuery::Balance {
                        address: account.clone(),
                        denom: denom.clone(),
                    }))?;
                Ok(response.amount.amount)
            }
        }
    }

    /// @notice Update the asset amount to reflect the deliverable amount if the asset is
    /// to be transferred.
    /// @dev For example, if the asset is 1000 UST, and the tax for sending 1000 UST is
    /// 1 UST, then update amount to 1000 - 1 = 999.
    /// @dev Modified from
    /// https://github.com/terraswap/terraswap/blob/master/packages/terraswap/src/asset.rs#L58
    pub fn deduct_tax(
        &self,
        querier: &QuerierWrapper,
        amount: Uint128,
    ) -> StdResult<Uint128> {
        let tax = match self {
            Self::Token {
                ..
            } => Uint128::zero(),
            Self::NativeToken {
                denom,
            } => {
                if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.clone())?.cap;
                    std::cmp::min(
                        amount.checked_sub(amount.multiply_ratio(
                            DECIMAL_FRACTION,
                            DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                        ))?,
                        tax_cap,
                    )
                }
            }
        };
        Ok(amount - tax)
    }
}
