use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use mars_core::asset::Asset as MarsAsset;
use mars_core::red_bank::msg::{ExecuteMsg, QueryMsg, ReceiveMsg};
use mars_core::red_bank::UserAssetDebtResponse;

use cw_asset::{Asset, AssetInfo};

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
                asset: to_mars_asset(&asset.info), // NOTE: to be replaced with `into` later
                amount: asset.amount,
                recipient: None,
            })?,
            funds: vec![],
        }))
    }

    /// Generate message for repaying a specified amount of asset
    pub fn repay_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Cw20(_) => Ok(asset.send_msg(
                &self.contract_addr,
                to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&ReceiveMsg::RepayCw20 {
                        on_behalf_of: None,
                    })?,
                })?,
            )?),
            AssetInfo::Native(denom) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&ExecuteMsg::RepayNative {
                    denom: denom.into(),
                    on_behalf_of: None,
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
        let response: UserAssetDebtResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&QueryMsg::UserAssetDebt {
                    user_address: user_address.to_string(),
                    asset: to_mars_asset(asset_info), // NOTE: to be replaced with `into` later
                })?,
            }))?;
        Ok(response.amount)
    }
}

/// Cast `cw_asset::AssetInfo` to `mars_core::asset::Asset`
///
/// NOTE: Once `mars-core` is open sourced and published on crates.io, an `Into<MarsAsset>` trait
/// will be implemented for `cw_asset::AssetInfo`. This helper function can be removed following that
fn to_mars_asset(info: &AssetInfo) -> MarsAsset {
    match info {
        AssetInfo::Cw20(contract_addr) => MarsAsset::Cw20 {
            contract_addr: contract_addr.to_string(),
        },
        AssetInfo::Native(denom) => MarsAsset::Native {
            denom: denom.clone(),
        },
    }
}
