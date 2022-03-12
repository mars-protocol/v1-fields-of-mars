use cosmwasm_std::{
    to_binary, Addr, Api, Decimal, QuerierWrapper, QueryRequest, StdResult, WasmQuery,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use mars_core::math::decimal::Decimal as MarsDecimal;
use mars_core::oracle::msg::QueryMsg;

use cw_asset::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OracleBase<T> {
    pub contract_addr: T,
}

pub type OracleUnchecked = OracleBase<String>;
pub type Oracle = OracleBase<Addr>;

impl From<Oracle> for OracleUnchecked {
    fn from(oracle: Oracle) -> Self {
        OracleUnchecked {
            contract_addr: oracle.contract_addr.to_string(),
        }
    }
}

impl OracleUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Oracle> {
        Ok(Oracle {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        })
    }
}

impl Oracle {
    pub fn query_price(
        &self,
        querier: &QuerierWrapper,
        asset_info: &AssetInfo,
    ) -> StdResult<Decimal> {
        let response: MarsDecimal = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&QueryMsg::AssetPriceByReference {
                asset_reference: get_asset_reference(asset_info),
            })?,
        }))?;
        Ok(response.to_std_decimal()) // cast mars_core::math::decimal::Decimal to cosmwasm_std::Decimal
    }
}

fn get_asset_reference(asset_info: &AssetInfo) -> Vec<u8> {
    match asset_info {
        AssetInfo::Cw20(contract_addr) => contract_addr.as_bytes().to_vec(),
        AssetInfo::Native(denom) => denom.as_bytes().to_vec(),
    }
}