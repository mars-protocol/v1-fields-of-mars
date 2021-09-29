use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use red_bank::msg::{DebtResponse, ExecuteMsg, QueryMsg, ReceiveMsg};

use crate::adapters::{Asset, AssetInfo};

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

impl RedBankUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<RedBank> {
        Ok(RedBank {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        })
    }
}

impl RedBank {
    /// Generate message for borrowing a specified amount of asset
    pub fn borrow_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Borrow {
                asset: asset.info.clone().into(),
                amount: asset.amount,
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate message for repaying a specified amount of asset
    /// @dev Note: we do not deduct tax here
    pub fn repay_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Cw20 {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&ReceiveMsg::RepayCw20 {})?,
                })?,
            })),
            AssetInfo::Native {
                denom,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&ExecuteMsg::RepayNative {
                    denom: denom.into(),
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
        asset_info: &AssetInfo,
    ) -> StdResult<Uint128> {
        let response: DebtResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&QueryMsg::UserDebt {
                user_address: user_address.to_string(),
            })?,
        }))?;

        let amount = response
            .debts
            .iter()
            .find(|debt| debt.denom == asset_info.get_denom())
            .map(|debt| debt.amount_scaled)
            .unwrap_or_else(Uint128::zero);

        Ok(amount)
    }
}
