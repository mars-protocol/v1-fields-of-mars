use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use field_of_mars::red_bank::MockInstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub mock_interest_rate: Decimal256,
}

impl Config {
    pub fn new(msg: MockInstantiateMsg) -> Self {
        Self {
            mock_interest_rate: msg.mock_interest_rate.unwrap_or(Decimal256::one()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub borrowed_amount: Uint256,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            borrowed_amount: Uint256::zero(),
        }
    }
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const POSITION: Map<(&Addr, &str), Position> = Map::new("position");
