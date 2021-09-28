use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use crate::msg::ConfigBase;

pub type Config = ConfigBase<Addr>;

pub const CONFIG: Item<Config> = Item::new("config");
pub const BOND_AMOUNT: Map<&Addr, Uint128> = Map::new("bond_amount");
