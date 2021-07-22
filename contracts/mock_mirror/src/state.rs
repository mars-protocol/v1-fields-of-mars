use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use field_of_mars::staking::mirror_staking::MockInstantiateMsg;

pub type Config = MockInstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub bond_amount: Uint128,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            bond_amount: Uint128::zero(),
        }
    }
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const POSITION: Map<&Addr, Position> = Map::new("position");
