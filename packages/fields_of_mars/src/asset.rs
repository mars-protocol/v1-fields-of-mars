use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankMsg, BankQuery, Coin, CosmosMsg, MessageInfo,
    QuerierWrapper, QueryRequest, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use terra_cosmwasm::TerraQuerier;

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfo<T> {
    Token { contract_addr: T },
    NativeToken { denom: String },
}

pub type AssetInfoUnchecked = AssetInfo<String>;
pub type AssetInfoChecked = AssetInfo<Addr>;

impl From<AssetInfoChecked> for AssetInfoUnchecked {
    fn from(asset_info: AssetInfoChecked) -> Self {
        match &asset_info {
            AssetInfoChecked::Token { contract_addr } => AssetInfoUnchecked::Token {
                contract_addr: contract_addr.to_string(),
            },
            AssetInfoChecked::NativeToken { denom } => AssetInfoUnchecked::NativeToken {
                denom: denom.clone(),
            },
        }
    }
}

impl AssetInfoUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetInfoChecked> {
        let checked = match self {
            AssetInfoUnchecked::Token { contract_addr } => AssetInfoChecked::Token {
                contract_addr: api.addr_validate(contract_addr)?,
            },
            AssetInfoUnchecked::NativeToken { denom } => AssetInfoChecked::NativeToken {
                denom: denom.clone(),
            },
        };

        Ok(checked)
    }
}

impl AssetInfoChecked {
    /// Get the asset's label, which is used in `red_bank::msg::DebtResponse`
    /// For native tokens, it's the denom, e.g. uusd, uluna
    /// For CW20 tokens, it's the contract address
    pub fn get_denom(&self) -> String {
        match self {
            AssetInfo::Token { contract_addr } => contract_addr.to_string(),
            AssetInfo::NativeToken { denom } => denom.clone(),
        }
    }

    /// Get the asset's reference, used in `oracle::msg::QueryMsg::AssetPriceByReference`
    pub fn get_reference(&self) -> Vec<u8> {
        self.get_denom().as_bytes().to_vec()
    }

    /// @notice Query an account's balance of the specified asset
    pub fn query_balance(&self, querier: &QuerierWrapper, account: &Addr) -> StdResult<Uint128> {
        match self {
            AssetInfoChecked::Token { contract_addr } => {
                let response: Cw20BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.to_string(),
                        msg: to_binary(&Cw20QueryMsg::Balance {
                            address: account.to_string(),
                        })?,
                    }))?;
                Ok(response.balance)
            }
            AssetInfoChecked::NativeToken { denom } => {
                let response: BalanceResponse =
                    querier.query(&QueryRequest::Bank(BankQuery::Balance {
                        address: account.to_string(),
                        denom: denom.clone(),
                    }))?;
                Ok(response.amount.amount)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Asset<T> {
    pub info: T,
    pub amount: Uint128,
}

pub type AssetUnchecked = Asset<AssetInfoUnchecked>;
pub type AssetChecked = Asset<AssetInfoChecked>;

impl From<AssetChecked> for AssetUnchecked {
    fn from(asset: AssetChecked) -> Self {
        AssetUnchecked {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

impl AssetUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetChecked> {
        let checked = AssetChecked {
            info: self.info.check(api)?,
            amount: self.amount,
        };

        Ok(checked)
    }
}

impl AssetChecked {
    /// Check if native token of specified amount was sent along a message
    /// Skip if asset if CW20
    pub fn assert_sent_fund(&self, message: &MessageInfo) -> StdResult<()> {
        let denom = match &self.info {
            AssetInfoChecked::Token { .. } => {
                return Ok(());
            }
            AssetInfoChecked::NativeToken { denom } => denom,
        };

        let sent_amount = match message.funds.iter().find(|fund| &fund.denom == denom) {
            Some(fund) => fund.amount,
            None => Uint128::zero(),
        };

        if sent_amount != self.amount {
            return Err(StdError::generic_err(format!(
                "Sent fund mismatch! denom: {} expected: {} received: {}",
                denom, self.amount, sent_amount
            )));
        }

        Ok(())
    }

    /// Generate the message for transferring asset of a specific amount from one account
    /// to another using the `Cw20ExecuteMsg::Transfer` message type
    ///
    /// NOTE: `amount` must have tax deducted BEFORE passing into this function!
    ///
    /// Usage:
    /// let msg = asset.deduct_tax(deps.querier)?.transfer_msg(to, amount)?;
    pub fn transfer_msg(&self, to: &Addr) -> StdResult<CosmosMsg> {
        match &self.info {
            AssetInfoChecked::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: to.to_string(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfoChecked::NativeToken { denom } => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: to.to_string(),
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount: self.amount,
                }],
            })),
        }
    }

    /// Generate the message for transferring asset of a specific amount from one account
    /// to another using the `Cw20HandleMsg::TransferFrom` message type
    ///
    /// NOTE: Must have allowance
    pub fn transfer_from_msg(&self, from: &Addr, to: &Addr) -> StdResult<CosmosMsg> {
        match &self.info {
            AssetInfoChecked::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: from.to_string(),
                    recipient: to.to_string(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfoChecked::NativeToken { .. } => Err(StdError::generic_err(
                "`TransferFrom` does not apply to native tokens",
            )),
        }
    }

    /// Compute total cost (tax included) for transferring specified amount of asset
    ///
    /// E.g. If tax incurred for transferring 100 UST is 0.5 UST, then return 100.5 UST.
    /// This is the total amount that will be deducted from the sender's account.
    pub fn add_tax(&self, querier: &QuerierWrapper, amount: Uint128) -> StdResult<Self> {
        let tax = match &self.info {
            AssetInfoChecked::Token { .. } => Uint128::zero(),
            AssetInfoChecked::NativeToken { denom } => {
                if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.clone())?.cap;
                    std::cmp::min(amount * tax_rate, tax_cap)
                }
            }
        };

        Ok(AssetChecked {
            info: self.info.clone(),
            amount: self.amount + tax,
        })
    }

    /// Update the asset amount to reflect the deliverable amount if the asset is to be transferred.
    ///
    /// @dev Modified from
    /// https://github.com/terraswap/terraswap/blob/master/packages/terraswap/src/asset.rs#L58
    pub fn deduct_tax(&self, querier: &QuerierWrapper, amount: Uint128) -> StdResult<Self> {
        let tax = match &self.info {
            AssetInfoChecked::Token { .. } => Uint128::zero(),
            AssetInfoChecked::NativeToken { denom } => {
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

        Ok(AssetChecked {
            info: self.info.clone(),
            amount: self.amount - tax, // `tax` is guaranteed to be smaller than `amount` so no need to handle underflow
        })
    }
}
