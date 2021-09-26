use cw_storage_plus::Map;

use fields_of_mars::oracle::mock_msg::PriceSourceChecked;

pub const PRICE_SOURCE: Map<&[u8], PriceSourceChecked> = Map::new("price_source");
