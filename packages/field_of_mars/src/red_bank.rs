use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, Coin, CosmosMsg, Extern, HumanAddr, Querier,
    QueryRequest, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MockInitMsg {
    // User's debt = deposit_amount * mock_interest_rate
    pub mock_interest_rate: Option<Decimal256>,
}

pub type MockMigrateMsg = MockInitMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    Borrow {
        asset: RedBankAsset,
        amount: Uint256,
    },
    RepayNative {
        denom: String,
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
        address: HumanAddr,
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
        contract_addr: HumanAddr,
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
        match &info {
            AssetInfo::Token {
                contract_addr,
            } => Self::Cw20 {
                contract_addr: HumanAddr::from(contract_addr),
            },
            AssetInfo::NativeToken {
                denom,
            } => Self::Native {
                denom: String::from(denom),
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
    pub contract_addr: HumanAddr,
}

impl RedBank {
    /// @notice Convert `RedBank` to `RedBankRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<RedBankRaw> {
        Ok(RedBankRaw {
            contract_addr: deps.api.canonical_address(&self.contract_addr)?,
        })
    }

    /// @notice Generate message for borrowing a specified amount of asset
    pub fn borrow_message(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.clone(),
            send: vec![],
            msg: to_binary(&HandleMsg::Borrow {
                asset: RedBankAsset::from(asset),
                amount: Uint256::from(asset.amount),
            })?,
        }))
    }

    /// @notice Generate message for repaying a specified amount of asset
    /// @dev Note: we do not deduct tax here
    pub fn repay_message(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: self.contract_addr.clone(),
                    amount: asset.amount,
                    msg: Some(to_binary(&ReceiveMsg::RepayCw20 {})?),
                })?,
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.clone(),
                send: vec![Coin {
                    denom: String::from(denom),
                    amount: asset.amount,
                }],
                msg: to_binary(&HandleMsg::RepayNative {
                    denom: String::from(denom),
                })?,
            })),
        }
    }

    /// @notice Query the amount of debt a borrower owes to Red Bank
    pub fn query_debt<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        borrower: &HumanAddr,
        info: &AssetInfo,
    ) -> StdResult<Uint128> {
        let response: DebtResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(&QueryMsg::Debt {
                    address: HumanAddr::from(borrower),
                })?,
            }))?;

        match response
            .debts
            .iter()
            .find(|debt| debt.denom == info.query_denom(deps).unwrap())
        {
            Some(debt) => Ok(debt.amount.into()),
            None => Ok(Uint128::zero()),
        }
    }
}

//----------------------------------------------------------------------------------------
// Raw Type
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBankRaw {
    /// Address of Mars liquidity pool
    pub contract_addr: CanonicalAddr,
}

impl RedBankRaw {
    /// @notice Convert `RedBankRaw` to `RedBank`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<RedBank> {
        Ok(RedBank {
            contract_addr: deps.api.human_address(&self.contract_addr)?,
        })
    }
}
