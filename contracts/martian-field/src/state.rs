use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use fields_of_mars::martian_field::{Config, Position, State};

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const POSITION: Map<&Addr, Position> = Map::new("position");

// When sending a submsg, we need to store the user's address somewhere so that it can be accessed
// when handleing the reply
pub const TEMP_USER_ADDR: Item<Addr> = Item::new("temp_user_addr");
