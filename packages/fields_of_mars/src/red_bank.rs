use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{AssetChecked, AssetInfoChecked};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBank<T> {
    pub contract_addr: T,
}

pub type RedBankUnchecked = RedBank<String>;
pub type RedBankChecked = RedBank<Addr>;

impl From<RedBankChecked> for RedBankUnchecked {
    fn from(checked: RedBankChecked) -> Self {
        RedBankUnchecked {
            contract_addr: checked.contract_addr.to_string(),
        }
    }
}

impl RedBankUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<RedBankChecked> {
        let checked = RedBankChecked {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        };

        Ok(checked)
    }
}

impl RedBankChecked {
    /// Generate message for borrowing a specified amount of asset
    pub fn borrow_msg(&self, asset: &AssetChecked) -> StdResult<CosmosMsg> {
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
    pub fn repay_msg(&self, asset: &AssetChecked) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfoChecked::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&msg::ReceiveMsg::RepayCw20 {})?,
                })?,
            })),
            AssetInfoChecked::NativeToken { denom } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
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

    pub fn query_user_debt(
        &self,
        querier: &QuerierWrapper,
        user_address: &Addr,
        asset_info: &AssetInfoChecked,
    ) -> StdResult<Uint128> {
        let response: msg::DebtResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&msg::QueryMsg::UserDebt {
                user_address: user_address.to_string(),
            })?,
        }))?;

        let amount = response
            .debts
            .iter()
            .find(|debt| debt.denom == asset_info.get_denom())
            .map(|debt| debt.amount)
            .unwrap_or_else(Uint128::zero);

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
        UserDebt { user_address: String },
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

    impl From<&AssetInfoChecked> for RedBankAsset {
        fn from(info: &AssetInfoChecked) -> Self {
            match info {
                AssetInfoChecked::Token { contract_addr } => Self::Cw20 {
                    contract_addr: contract_addr.to_string(),
                },
                AssetInfoChecked::NativeToken { denom } => Self::Native {
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
            asset: msg::RedBankAsset,
            amount: Uint128,
        },
        RepayNative {
            denom: String,
        },
        /// NOTE: Only used in mock contract! Not present in actual Red Bank contract
        /// Forcibly set a user's debt amount. Used in tests to simulate the accrual of debts
        SetUserDebt {
            user_address: String,
            denom: String,
            amount: Uint128,
        },
    }
}
