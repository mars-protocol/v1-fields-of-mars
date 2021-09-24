use cosmwasm_std::{to_binary, Addr, CosmosMsg, Decimal, StdResult, Timestamp, Uint128, WasmMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{AssetInfoUnchecked, AssetUnchecked};
use crate::oracle::OracleUnchecked;
use crate::pool::PoolUnchecked;
use crate::red_bank::RedBankUnchecked;
use crate::staking::StakingUnchecked;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// Info of the asset to be deposited by the user
    pub primary_asset_info: AssetInfoUnchecked,
    /// Info of the asset to be either deposited by user or borrowed from Mars
    pub secondary_asset_info: AssetInfoUnchecked,
    /// Mars money market aka Red Bank
    pub red_bank: RedBankUnchecked,
    /// Mars oracle contract
    pub oracle: OracleUnchecked,
    /// Astroport pool of primary/secondary assets
    pub pool: PoolUnchecked,
    /// Staking contract where LP tokens can be bonded to earn rewards
    pub staking: StakingUnchecked,
    /// Accounts who can harvest
    pub keepers: Vec<String>,
    /// Account to receive fee payments
    pub treasury: String,
    /// Account who can update config
    pub governance: String,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum ExecuteMsg {
    /// Open a new position or add to an existing position
    IncreasePosition { deposits: [AssetUnchecked; 2] },
    /// Reduce a position, or close it completely
    ReducePosition {
        bond_units: Option<Uint128>,
        swap: Decimal,
        repay: bool,
    },
    /// Pay down debt owed to Mars, reduce debt units
    PayDebt {
        user: Option<String>,
        deposit: AssetUnchecked,
    },
    /// Claim staking reward and reinvest
    Harvest {},
    /// Close an unhealthy position in order to liquidate it
    /// NOTE: This function is for liquidators. Users who wish to close their healthy
    /// positions use `ExecuteMsg::ReducePosition`
    ClosePosition { user: String },
    /// Pay down remaining debt of a closed position and be awarded its unlocked assets
    Liquidate {
        user: String,
        deposit: AssetUnchecked,
    },
    /// Update data stored in config (owner only)
    UpdateConfig { new_config: InstantiateMsg },
    /// Callbacks; only callable by the strategy itself.
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    /// Provide the user's unlocked primary/secondary assets to the AMM, receive share tokens
    ProvideLiquidity { user: Addr },
    /// Burn the user's unlocked share tokens, receive primary/secondary assets
    RemoveLiquidity { user: Addr },
    /// Bond share tokens to the staking contract
    Bond { user: Addr },
    /// Unbond share tokens from the staking contract
    Unbond {
        user: Addr,
        bond_units: Option<Uint128>,
    },
    /// Borrow specified amount of short asset from Mars
    Borrow { user: Addr, amount: Uint128 },
    /// Use the user's unlocked short asset to repay debt
    Repay { user: Addr },
    /// Send a percentage of a user's unlocked assets to a specified recipient
    Refund {
        user: Addr,
        recipient: Addr,
        percentage: Decimal,
    },
    /// Collect a portion of rewards as performance fee, swap half of the rest for UST
    Reinvest { amount: Uint128 },
    /// Save a snapshot of a user's position; useful for the frontend to calculate PnL
    Snapshot { user: Addr },
    /// Check if a user's LTV is below liquidation threshold; throw an error if not
    AssertHealth { user: Addr },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn into_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: String::from(contract_addr),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Return strategy configurations
    Config {},
    /// Return the global state of the strategy
    State {},
    /// Return data on an individual user's position
    Position { user: String },
    /// Query the health of a user's position: value of assets, debts, and LTV
    Health { user: String },
    /// Snapshot of a user's position the last time the position was increased, decreased,
    /// or when debt was paid. Useful for the frontend to calculate PnL
    Snapshot { user: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

pub type ConfigResponse = InstantiateMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: [AssetUnchecked; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct HealthResponse {
    /// Value of the position's asset, measured in the short asset
    pub bond_value: Uint128,
    /// Value of the position's debt, measured in the short asset
    pub debt_value: Uint128,
    /// The ratio of `debt_value` to `bond_value`; None if `bond_value` is zero
    pub ltv: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SnapshotResponse {
    /// UNIX timestamp at which the snapshot was taken
    pub time: Timestamp,
    /// Block number at which the snapshot was taken
    pub height: u64,
    /// Snapshot of the position's health info
    pub health: HealthResponse,
    /// Snapshot of the position
    pub position: PositionResponse,
}
