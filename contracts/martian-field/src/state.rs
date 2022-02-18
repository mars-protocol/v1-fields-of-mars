use cosmwasm_std::{Addr};
use cw_storage_plus::{Item, Map};

use fields_of_mars::martian_field::{Config, Position, Snapshot, State};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const POSITION: Map<&Addr, Position> = Map::new("position");

// save user address temporarily between callbacks
pub const CACHED_USER_ADDR: Item<Addr> = Item::new("cached_user_addr");

// snapshot is used by the frontend calculate user PnL. once we build a transaction indexer that can
// calculate PnL without relying on on-chain snapshots, this will be removed
pub const SNAPSHOT: Map<&Addr, Snapshot> = Map::new("snapshot");