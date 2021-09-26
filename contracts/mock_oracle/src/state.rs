use cw_storage_plus::Map;

use fields_of_mars::oracle::mock_msg::PriceSourceChecked;

// key: asset_reference
// value: price_source (checked)
pub const PRICE_SOURCE: Map<&[u8], PriceSourceChecked> = Map::new("price_source");
