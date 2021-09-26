use cosmwasm_std::Addr;
use cw_storage_plus::Item;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub pair_address: Addr,
    pub token_address: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");
