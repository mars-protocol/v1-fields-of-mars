use cosmwasm_std::{Addr, Uint128};
use cw_asset::AssetList;
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};

use fields_of_mars::martian_field::Config;

pub const CONFIG: Item<Config> = Item::new("config");

#[derive(Serialize, Deserialize)]
pub struct State {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
    /// Reward tokens that can be reinvested in the next harvest
    pub pending_rewards: AssetList,
}

pub const STATE: Item<State> = Item::new("state");

#[derive(Serialize, Deserialize)]
pub struct Position {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: AssetList,
}

pub const POSITION: Map<&Addr, Position> = Map::new("position");
