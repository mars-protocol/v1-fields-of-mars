use cosmwasm_std::{
    to_binary, Addr, Api, Decimal, QuerierWrapper, QueryRequest, StdResult, WasmQuery,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{AssetInfoChecked, AssetInfoUnchecked};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Oracle<T> {
    pub contract_addr: T,
}

pub type OracleUnchecked = Oracle<String>;
pub type OracleChecked = Oracle<Addr>;

impl From<OracleChecked> for OracleUnchecked {
    fn from(checked: OracleChecked) -> Self {
        OracleUnchecked {
            contract_addr: checked.contract_addr.to_string(),
        }
    }
}

impl OracleUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<OracleChecked> {
        let checked = OracleChecked {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        };

        Ok(checked)
    }
}

impl OracleChecked {
    /// NOTE: For now, we don't check whether the price data is too old by verifying `last_updated`.
    /// We might want to do this in a future version
    pub fn query_price(
        &self,
        querier: &QuerierWrapper,
        asset_info: &AssetInfoChecked,
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
        asset_infos: &[AssetInfoChecked],
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
    pub enum PriceSource<T> {
        Fixed { price: Decimal },
        AstroportSpot { pair_address: T, asset_address: T },
    }

    pub type PriceSourceUnchecked = PriceSource<String>;
    pub type PriceSourceChecked = PriceSource<Addr>;

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
