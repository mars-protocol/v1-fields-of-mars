use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

use crate::msg::ConfigBase;

pub type Config = ConfigBase<Addr>;

pub const CONFIG: Item<Config> = Item::new("config");
pub const STAKING_TOKEN: Map<&Addr, Addr> = Map::new("staking_token");
pub const BOND_AMOUNT: Map<(&Addr, &Addr), Uint128> = Map::new("bond_amount");
