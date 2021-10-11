use cw_storage_plus::Map;

use mars_core::oracle::PriceSourceChecked;

// key: asset_reference
// value: price_source (checked)
pub const PRICE_SOURCE: Map<&[u8], PriceSourceChecked> = Map::new("price_source");
