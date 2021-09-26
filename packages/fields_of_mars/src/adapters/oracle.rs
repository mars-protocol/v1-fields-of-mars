use cosmwasm_std::{
    to_binary, Addr, Api, Decimal, QuerierWrapper, QueryRequest, StdResult, WasmQuery,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use mars::oracle::msg::{AssetPriceResponse, QueryMsg};

use crate::adapters::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct OracleBase<T> {
    pub contract_addr: T,
}

pub type OracleUnchecked = OracleBase<String>;
pub type Oracle = OracleBase<Addr>;

impl From<Oracle> for OracleUnchecked {
    fn from(checked: Oracle) -> Self {
        OracleUnchecked {
            contract_addr: checked.contract_addr.to_string(),
        }
    }
}

impl OracleUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Oracle> {
        let checked = Oracle {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        };

        Ok(checked)
    }
}

impl Oracle {
    /// NOTE: For now, we don't check whether the price data is too old by verifying `last_updated`.
    /// We might want to do this in a future version
    pub fn query_price(
        &self,
        querier: &QuerierWrapper,
        asset_info: &AssetInfo,
    ) -> StdResult<Decimal> {
        let response: AssetPriceResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&QueryMsg::AssetPriceByReference {
                    asset_reference: asset_info.get_reference(),
                })?,
            }))?;

        Ok(response.price)
    }

    pub fn query_prices(
        &self,
        querier: &QuerierWrapper,
        asset_infos: &[AssetInfo],
    ) -> StdResult<Vec<Decimal>> {
        Ok(asset_infos
            .iter()
            .map(|asset_info| self.query_price(querier, asset_info).unwrap())
            .collect())
    }
}
