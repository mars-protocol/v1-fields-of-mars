use cosmwasm_bignumber::Uint256;
use cosmwasm_std::Addr;
use cw_storage_plus::Map;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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

pub const POSITION: Map<(&Addr, &str), Position> = Map::new("position");
