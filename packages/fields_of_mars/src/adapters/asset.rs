use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankMsg, BankQuery, Coin, CosmosMsg, MessageInfo,
    QuerierWrapper, QueryRequest, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};

use terra_cosmwasm::TerraQuerier;

use astroport;
use mars;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfoBase<T> {
    Cw20 { contract_addr: T },
    Native { denom: String },
}

pub type AssetInfoUnchecked = AssetInfoBase<String>;
pub type AssetInfo = AssetInfoBase<Addr>;

impl From<AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: AssetInfo) -> Self {
        match &asset_info {
            AssetInfo::Cw20 { contract_addr } => AssetInfoUnchecked::Cw20 {
                contract_addr: contract_addr.to_string(),
            },
            AssetInfo::Native { denom } => AssetInfoUnchecked::Native {
                denom: denom.clone(),
            },
        }
    }
}

impl AssetInfoUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetInfo> {
        let checked = match self {
            AssetInfoUnchecked::Cw20 { contract_addr } => AssetInfo::Cw20 {
                contract_addr: api.addr_validate(contract_addr)?,
            },
            AssetInfoUnchecked::Native { denom } => AssetInfo::Native {
                denom: denom.clone(),
            },
        };

        Ok(checked)
    }
}

impl From<AssetInfo> for mars::asset::Asset {
    fn from(asset_info: AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20 { contract_addr } => Self::Cw20 {
                contract_addr: contract_addr.to_string(),
            },
            AssetInfo::Native { denom } => Self::Native { denom },
        }
    }
}

impl From<AssetInfo> for astroport::asset::AssetInfo {
    fn from(asset_info: AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20 { contract_addr } => Self::Token { contract_addr },
            AssetInfo::Native { denom } => Self::NativeToken { denom },
        }
    }
}

impl PartialEq<AssetInfo> for astroport::asset::AssetInfo {
    fn eq(&self, other: &AssetInfo) -> bool {
        match self {
            Self::Token { contract_addr } => {
                let self_contract_addr = contract_addr;
                if let AssetInfo::Cw20 { contract_addr } = other {
                    self_contract_addr == contract_addr
                } else {
                    false
                }
            }
            Self::NativeToken { denom } => {
                let self_denom = denom;
                if let AssetInfo::Native { denom } = other {
                    self_denom == denom
                } else {
                    false
                }
            }
        }
    }
}

impl AssetInfo {
    // INSTANCE CREATION

    pub fn cw20(contract_addr: &Addr) -> Self {
        Self::Cw20 {
            contract_addr: contract_addr.clone(),
        }
    }

    pub fn native(denom: &dyn ToString) -> Self {
        Self::Native {
            denom: denom.to_string(),
        }
    }

    // UTILITIES

    /// Get the asset's label, which is used in `red_bank::msg::DebtResponse`
    /// For native tokens, it's the denom, e.g. uusd, uluna
    /// For CW20 tokens, it's the contract address
    pub fn get_denom(&self) -> String {
        match self {
            AssetInfo::Cw20 { contract_addr } => contract_addr.to_string(),
            AssetInfo::Native { denom } => denom.clone(),
        }
    }

    /// Get the asset's reference, used in `oracle::msg::QueryMsg::AssetPriceByReference`
    pub fn get_reference(&self) -> Vec<u8> {
        self.get_denom().as_bytes().to_vec()
    }

    // QUERIES

    /// Query an account's balance of the specified asset
    pub fn query_balance(&self, querier: &QuerierWrapper, account: &Addr) -> StdResult<Uint128> {
        match self {
            AssetInfo::Cw20 { contract_addr } => {
                let response: Cw20BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.to_string(),
                        msg: to_binary(&Cw20QueryMsg::Balance {
                            address: account.to_string(),
                        })?,
                    }))?;
                Ok(response.balance)
            }
            AssetInfo::Native { denom } => {
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
pub struct AssetBase<T> {
    pub info: T,
    pub amount: Uint128,
}

pub type AssetUnchecked = AssetBase<AssetInfoUnchecked>;
pub type Asset = AssetBase<AssetInfo>;

impl From<Asset> for AssetUnchecked {
    fn from(asset: Asset) -> Self {
        AssetUnchecked {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

impl AssetUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Asset> {
        let checked = Asset {
            info: self.info.check(api)?,
            amount: self.amount,
        };

        Ok(checked)
    }
}

impl From<Asset> for astroport::asset::Asset {
    fn from(asset: Asset) -> Self {
        Self {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

impl Asset {
    // INSTANCE CREATION

    pub fn cw20<A: Into<Uint128>>(contract_addr: &Addr, amount: A) -> Self {
        Asset {
            info: AssetInfo::cw20(contract_addr),
            amount: amount.into(),
        }
    }

    pub fn native<A: Into<Uint128>>(denom: &dyn ToString, amount: A) -> Self {
        Asset {
            info: AssetInfo::native(denom),
            amount: amount.into(),
        }
    }

    // MESSAGES

    /// Generate the message for transferring asset of a specific amount from one account
    /// to another using the `Cw20ExecuteMsg::Transfer` message type
    ///
    /// NOTE: `amount` must have tax deducted BEFORE passing into this function!
    ///
    /// Usage:
    /// let msg = asset.deduct_tax(deps.querier)?.transfer_msg(to, amount)?;
    pub fn transfer_msg(&self, to: &Addr) -> StdResult<CosmosMsg> {
        match &self.info {
            AssetInfo::Cw20 { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: to.to_string(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native { denom } => Ok(CosmosMsg::Bank(BankMsg::Send {
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
            AssetInfo::Cw20 { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: from.to_string(),
                    recipient: to.to_string(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native { .. } => Err(StdError::generic_err(
                "`TransferFrom` does not apply to native tokens",
            )),
        }
    }

    // UTILITIES

    /// Check if native token of specified amount was sent along a message
    /// Skip if asset if CW20
    pub fn assert_sent_fund(&self, message: &MessageInfo) -> StdResult<()> {
        let denom = match &self.info {
            AssetInfo::Cw20 { .. } => {
                return Ok(());
            }
            AssetInfo::Native { denom } => denom,
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

    /// Compute total cost (tax included) for transferring specified amount of asset
    ///
    /// E.g. If tax incurred for transferring 100 UST is 0.5 UST, then return 100.5 UST.
    /// This is the total amount that will be deducted from the sender's account.
    pub fn add_tax(&self, querier: &QuerierWrapper, amount: Uint128) -> StdResult<Self> {
        let tax = match &self.info {
            AssetInfo::Cw20 { .. } => Uint128::zero(),
            AssetInfo::Native { denom } => {
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

        Ok(Asset {
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
            AssetInfo::Cw20 { .. } => Uint128::zero(),
            AssetInfo::Native { denom } => {
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

        Ok(Asset {
            info: self.info.clone(),
            amount: self.amount - tax, // `tax` is guaranteed to be smaller than `amount` so no need to handle underflow
        })
    }
}
