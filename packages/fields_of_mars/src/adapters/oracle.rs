use cosmwasm_std::{
    to_binary, Addr, Api, Decimal, QuerierWrapper, QueryRequest, StdResult, WasmQuery,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{AssetInfo, AssetInfoUnchecked};

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
        let response: msg::AssetPriceResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&msg::QueryMsg::AssetPriceByReference {
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

pub mod msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        AssetPriceByReference { asset_reference: Vec<u8> },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct AssetPriceResponse {
        pub price: Decimal,
        pub last_updated: u64,
    }
}

pub mod mock_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum PriceSourceBase<T> {
        Fixed { price: Decimal },
        AstroportSpot { pair_address: T, asset_address: T },
    }

    pub type PriceSourceUnchecked = PriceSourceBase<String>;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct InstantiateMsg {}

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        SetAsset {
            asset_info: AssetInfoUnchecked,
            price_source: PriceSourceUnchecked,
        },
    }
}
