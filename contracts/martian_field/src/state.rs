use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use field_of_mars::martian_field;

pub type Config = martian_field::ConfigResponse;
pub type State = martian_field::StateResponse;
pub type Position = martian_field::PositionResponse;
pub type Snapshot = martian_field::SnapshotResponse;

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const POSITION: Map<&Addr, Position> = Map::new("position");
pub const SNAPSHOT: Map<&Addr, Snapshot> = Map::new("snapshot");
