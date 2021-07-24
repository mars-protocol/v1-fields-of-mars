use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    to_binary, Addr, Coin, QuerierWrapper, QueryRequest, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MockInstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Borrow {
        asset: RedBankAsset,
        amount: Uint256,
    },
    RepayNative {
        denom: String,
    },
    /// @notice Forcibly resets a user's debt amount. Used in tests to simulate the accrual
    /// of debts. The actual Red Bank doesn't have this message type
    SetDebt {
        user: String,
        denom: String,
        amount: Uint256,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    RepayCw20 {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Debt {
        address: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtResponse {
    pub debts: Vec<DebtInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtInfo {
    pub denom: String,
    pub amount: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RedBankAsset {
    Cw20 {
        contract_addr: String,
    },
    Native {
        denom: String,
    },
}

impl From<Asset> for RedBankAsset {
    fn from(asset: Asset) -> Self {
        Self::from(&asset)
    }
}

impl From<&Asset> for RedBankAsset {
    fn from(asset: &Asset) -> Self {
        Self::from(&asset.info)
    }
}

impl From<AssetInfo> for RedBankAsset {
    fn from(info: AssetInfo) -> Self {
        Self::from(&info)
    }
}

impl From<&AssetInfo> for RedBankAsset {
    fn from(info: &AssetInfo) -> Self {
        match info {
            AssetInfo::Token {
                contract_addr,
            } => Self::Cw20 {
                contract_addr: contract_addr.clone(),
            },
            AssetInfo::NativeToken {
                denom,
            } => Self::Native {
                denom: denom.clone(),
            },
        }
    }
}

//----------------------------------------------------------------------------------------
// Adapter
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBank {
    /// Address of Mars liquidity pool
    pub contract_addr: String,
}

impl RedBank {
    /// @notice Generate message for borrowing a specified amount of asset
    pub fn borrow_msg(&self, asset: &Asset) -> StdResult<SubMsg> {
        Ok(SubMsg::new(WasmMsg::Execute {
            contract_addr: self.contract_addr.clone(),
            msg: to_binary(&ExecuteMsg::Borrow {
                asset: RedBankAsset::from(asset),
                amount: Uint256::from(asset.amount),
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate message for repaying a specified amount of asset
    /// @dev Note: we do not deduct tax here
    pub fn repay_msg(&self, asset: &Asset) -> StdResult<SubMsg> {
        match &asset.info {
            AssetInfo::Token {
                contract_addr,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.clone(),
                    amount: asset.amount,
                    msg: to_binary(&ReceiveMsg::RepayCw20 {})?,
                })?,
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(&ExecuteMsg::RepayNative {
                    denom: denom.clone(),
                })?,
                funds: vec![Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }],
            })),
        }
    }

    /// @notice Query the amount of debt a borrower owes to Red Bank
    pub fn query_debt(
        &self,
        querier: &QuerierWrapper,
        borrower: &Addr,
        info: &AssetInfo,
    ) -> StdResult<Uint128> {
        let response: DebtResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(&QueryMsg::Debt {
                    address: String::from(borrower),
                })?,
            }))?;

        match response
            .debts
            .iter()
            .find(|debt| debt.denom == info.query_denom(querier).unwrap())
        {
            Some(debt) => Ok(debt.amount.into()),
            None => Ok(Uint128::zero()),
        }
    }
}
