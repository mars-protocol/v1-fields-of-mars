use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub anchor_token: Addr,
    pub staking_token: Addr,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const BOND_AMOUNT: Map<&Addr, Uint128> = Map::new("bond_amount");
