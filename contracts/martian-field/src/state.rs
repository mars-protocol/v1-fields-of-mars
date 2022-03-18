use cosmwasm_std::{Addr, Uint128};
use cw_asset::AssetList;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use fields_of_mars::martian_field::Config;

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
    /// Reward tokens that can be reinvested in the next harvest
    pub pending_rewards: AssetList,
}

// `Addr` does not have `Default` implemented, so we can't derive the Default trait
impl Default for State {
    fn default() -> Self {
        State {
            total_bond_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
            pending_rewards: AssetList::default(),
        }
    }
}

pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: AssetList,
}

// `Addr` does not have `Default` implemented, so we can't derive the Default trait
impl Default for Position {
    fn default() -> Self {
        Position {
            bond_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unlocked_assets: AssetList::default(),
        }
    }
}

impl From<State> for Position {
    fn from(state: State) -> Self {
        Position {
            bond_units: state.total_bond_units,
            debt_units: state.total_debt_units,
            unlocked_assets: state.pending_rewards,
        }
    }
}

impl Position {
    pub fn is_empty(self: &Position) -> bool {
        self.bond_units.is_zero() && self.debt_units.is_zero() && self.unlocked_assets.len() == 0
    }
}

pub const POSITION: Map<&Addr, Position> = Map::new("position");

// save user address temporarily between callbacks
pub const CACHED_USER_ADDR: Item<Addr> = Item::new("cached_user_addr");
