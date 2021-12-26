use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use crate::msg::Config;

pub const CONFIG: Item<Config> = Item::new("config");

// the amount of MIR-UST liquidity tokens deposited by the user
pub const DEPOSIT: Map<&Addr, Uint128> = Map::new("deposit");
