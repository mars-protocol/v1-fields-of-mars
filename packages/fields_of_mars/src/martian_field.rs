use cosmwasm_std::{
    to_binary, CosmosMsg, Decimal, HumanAddr, StdResult, Uint128, WasmMsg,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    asset::{Asset, AssetInfo},
    red_bank::RedBank,
    staking::Staking,
    swap::Swap,
};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Info of the asset to be deposited by the user
    pub long_asset: AssetInfo,
    /// Info of the asset to be either deposited by user or borrowed from Mars
    pub short_asset: AssetInfo,
    /// Mars liquidity pool aka Red Bank
    pub red_bank: RedBank,
    /// TerraSwap/Astroport pair of long/short assets
    pub swap: Swap,
    /// Staking contract where LP tokens can be bonded to earn rewards
    pub staking: Staking,
    /// Accounts who can harvest
    pub keepers: Vec<HumanAddr>,
    /// Account to receive fee payments
    pub treasury: HumanAddr,
    /// Account who can update config
    pub governance: HumanAddr,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Open a new position or add to an existing position
    /// @dev Increase the user's unlocked long/short asset amount
    /// @param deposits Assets to deposit
    IncreasePosition {
        deposits: [Asset; 2],
    },
    /// Reduce a position, or close it completely
    /// @param bond_units The amount of `bond_units` to burn; default to all
    /// @param remove Whether to burn the unbonded share tokens to remove assets from the
    /// AMM. If `false`, the user will be refunded the share token
    /// @param repay When `remove` is set to `true`, whether the short asset removed is to
    /// be used to repay the debt (`true`) or sent to the user (`false`)
    ReducePosition {
        bond_units: Option<Uint128>,
        remove: bool,
        repay: bool,
    },
    /// Close a position whose LTV is greater than the liquidation threshold; typically to
    /// be followed by a `HandleMsg::Liquidate` call.
    /// Note: to close healthy positions, use `HandleMsg::ReducePosition`
    ClosePosition {
        user: HumanAddr,
    },
    /// Pay down debt owed to Mars, reduce debt units
    /// @param user Address of the user whose `debt_units` are to be reduced; default to sender
    /// @param deposit Asset to be used to pay debt
    PayDebt {
        user: Option<HumanAddr>,
        deposit: Asset,
    },
    /// Claim staking reward and reinvest
    Harvest {},
    /// Close an underfunded position, pay down remaining debt and claim the collateral
    /// @param user Address of the user whose position is to be closed
    /// @param deposit Asset to be used to liquidate to position
    Liquidate {
        user: HumanAddr,
        deposit: Asset,
    },
    /// Update data stored in config (owner only)
    /// @param new_config The new config info to be stored
    UpdateConfig {
        new_config: InitMsg,
    },
    /// Callbacks; only callable by the strategy itself.
    Callback(CallbackMsg),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallbackMsg {
    /// Provide the user's unlocked long/short assets to the AMM, receive share tokens
    /// @dev Zero the user's unlocked long/short amounts, increase unlocked share amount
    /// @dev If used in harvesting, `user` should be set to the contract's address
    ProvideLiquidity {
        user: HumanAddr,
    },
    /// Burn the user's unlocked share tokens, receive long/short assets
    /// @dev Zero the user's unlocked share amount, increase unlocked long/short amounts
    /// @param shares Amount of shares to burn
    RemoveLiquidity {
        user: HumanAddr,
    },
    /// Bond share tokens to the staking contract
    /// @dev Zero the user's unlocked share amount, increase asset units
    /// @dev If used in harvesting, `user` should be set to the contract's address
    Bond {
        user: HumanAddr,
    },
    /// Unbond share tokens from the staking contract
    /// @dev Reduce the user's asset units, increase unlocked share amount
    Unbond {
        user: HumanAddr,
        bond_units: Option<Uint128>,
    },
    /// Borrow specified amount of short asset from Mars
    /// @dev Increase the user's debt units
    /// @param amount Amount of short asset to borrow
    Borrow {
        user: HumanAddr,
        amount: Uint128,
    },
    /// Use the user's unlocked short asset to repay debt
    /// @dev Zero the user's unlocked short asset amount; reduce debt units
    /// @param amount Amount of short asset to repay
    Repay {
        user: HumanAddr,
    },
    /// Collect a portion of rewards as performance fee, swap half of the rest for UST
    /// @param amount of reward asset to be collected fee and swapped
    Swap {
        amount: Uint128,
    },
    /// Send a percentage of a user's unlocked assets to a specified recipient
    /// @dev Reduce the user's unlocked assets by the specified percentage
    Refund {
        user: HumanAddr,
        recipient: HumanAddr,
        percentage: Decimal,
    },
    /// Save a snapshot of a user's position; useful for the frontend to calculate PnL
    Snapshot {
        user: HumanAddr,
    },
    /// Check if a user's LTV is below liquidation threshold; throw an error if not
    AssertHealth {
        user: HumanAddr,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn into_cosmos_msg(&self, contract_addr: &HumanAddr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from(contract_addr),
            msg: to_binary(&HandleMsg::Callback(self.clone()))?,
            send: vec![],
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
    Position {
        user: HumanAddr,
    },
    /// Query the health of a user's position. If address is not provided, then query the
    /// contract's overall health
    Health {
        user: Option<HumanAddr>,
    },
    /// Snapshot of a user's position the last time the position was increased, decreased,
    /// or when debt was paid. Useful for the frontend to calculate PnL
    Snapshot {
        user: HumanAddr,
    },
}

// Migration is not implemented for the current version
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

//----------------------------------------------------------------------------------------
// Response Types
//----------------------------------------------------------------------------------------

pub type ConfigResponse = InitMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    /// Whether the position is actively farming, or closed pending liquidation
    pub is_active: bool,
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in TerraSwap pool; pending refund or liquidation
    pub unlocked_assets: [Asset; 3],
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
    pub time: u64,
    /// Block number at which the snapshot was taken
    pub height: u64,
    /// Snapshot of the position's health info
    pub health: HealthResponse,
    /// Snapshot of the position
    pub position: PositionResponse,
}
