use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBankBase<T> {
    pub contract_addr: T,
}

pub type RedBankUnchecked = RedBankBase<String>;
pub type RedBank = RedBankBase<Addr>;

impl From<RedBank> for RedBankUnchecked {
    fn from(red_bank: RedBank) -> Self {
        RedBankUnchecked {
            contract_addr: red_bank.contract_addr.to_string(),
        }
    }
}

impl RedBank {
    pub fn from_unchecked(api: &dyn Api, red_bank_unchecked: RedBankUnchecked) -> StdResult<Self> {
        Ok(RedBank {
            contract_addr: api.addr_validate(&red_bank_unchecked.contract_addr)?,
        })
    }

    /// Generate message for borrowing a specified amount of asset
    pub fn borrow_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&msg::ExecuteMsg::Borrow {
                asset: (&asset.info).into(), // Convert Astroport Asset to Red Bank Asset
                amount: asset.amount,
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate message for repaying a specified amount of asset
    /// @dev Note: we do not deduct tax here
    pub fn repay_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&msg::ReceiveMsg::RepayCw20 {})?,
                })?,
            })),
            AssetInfo::NativeToken { denom } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&msg::ExecuteMsg::RepayNative {
                    denom: denom.clone(),
                })?,
                funds: vec![Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }],
            })),
        }
    }

    pub fn query_debt(
        &self,
        querier: &QuerierWrapper,
        borrower: &Addr,
        info: &AssetInfo,
    ) -> StdResult<Uint128> {
        let response: msg::DebtResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&msg::QueryMsg::Debt {
                address: String::from(borrower),
            })?,
        }))?;

        let amount = match response
            .debts
            .iter()
            .find(|debt| debt.denom == info.get_label())
        {
            Some(debt) => debt.amount,
            None => Uint128::zero(),
        };

        Ok(amount)
    }
}

pub mod msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        Receive(Cw20ReceiveMsg),
        Borrow {
            asset: RedBankAsset,
            amount: Uint128,
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
        Debt { address: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct DebtResponse {
        pub debts: Vec<DebtInfo>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct DebtInfo {
        pub denom: String,
        pub amount: Uint128,
    }

    /// @dev Mars uses a different `Asset` type from that used by TerraSwap & Astroport
    /// We implement methods to allow easy conversion between these two
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum RedBankAsset {
        Cw20 { contract_addr: String },
        Native { denom: String },
    }

    impl From<&AssetInfo> for RedBankAsset {
        fn from(info: &AssetInfo) -> Self {
            match info {
                AssetInfo::Token { contract_addr } => Self::Cw20 {
                    contract_addr: contract_addr.to_string(),
                },
                AssetInfo::NativeToken { denom } => Self::Native {
                    denom: denom.clone(),
                },
            }
        }
    }
}

pub mod mock_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct InstantiateMsg {}

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        Receive(Cw20ReceiveMsg),
        Borrow {
            asset: RedBankAsset,
            amount: Uint128,
        },
        RepayNative {
            denom: String,
        },
        /// NOTE: Only used in mock contract! Not present in actual Red Bank contract
        /// Forcibly set a user's debt amount. Used in tests to simulate the accrual of debts
        SetDebt {
            user: String,
            denom: String,
            amount: Uint128,
        },
    }
}
