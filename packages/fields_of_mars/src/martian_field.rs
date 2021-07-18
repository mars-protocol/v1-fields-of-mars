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
    pub performance_fee_rate: Decimal,
    /// Percentage of asset to be charged as liquidation fee
    pub liquidation_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Open a new position or add to an existing position
    /// @param deposits Assets to deposit
    IncreasePosition {
        deposits: [Asset; 2],
    },
    /// Reduce a position, or close it completely
    /// @param bond_units The amount of `bond_units` to burn
    ReducePosition {
        bond_units: Option<Uint128>,
    },
    /// Pay down debt owed to Mars, reduce debt units
    /// @param user Address of the user whose `debt_units` are to be reduced; default to sender
    /// @param deposit Asset to be used to pay debt
    PayDebt {
        user: Option<HumanAddr>,
        deposit: Asset,
    },
    /// Close an underfunded position, pay down remaining debt and claim the collateral
    /// @param user Address of the user whose position is to be closed
    /// @param deposit Asset to be used to liquidate to position
    Liquidate {
        user: HumanAddr,
        deposit: Asset,
    },
    /// Claim staking reward and reinvest
    Harvest {},
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
    /// Provide specified amounts of token and UST to the Terraswap pool, receive LP tokens
    ProvideLiquidity {
        user: Option<HumanAddr>,
        assets: [Asset; 2],
    },
    /// Burn LP tokens, remove the liquidity from Terraswap, receive token and UST
    RemoveLiquidity {
        user: HumanAddr,
    },
    /// Bond LP tokens to the staking contract
    Bond {
        user: Option<HumanAddr>,
    },
    /// Unbond LP tokens from the staking contract
    Unbond {
        user: HumanAddr,
        bond_units: Uint128,
    },
    /// Borrow UST as uncollateralized loan from Mars
    Borrow {
        user: HumanAddr,
        borrow_asset: Asset,
    },
    /// Pay specified amount of UST to Mars
    Repay {
        user: HumanAddr,
        repay_asset: Asset,
    },
    /// Collect a portion of rewards as performance fee, swap half of the rest for UST
    Swap {
        offer_asset: Asset,
    },
    /// Verify the user's debt ratio, then refund unstaked token and UST to the user
    Refund {
        user: HumanAddr,
        to: HumanAddr,
        percentage: Decimal,
    },
    /// Save a snapshot of a user's position; useful for the frontend to calculate PnL
    Snapshot {
        user: HumanAddr,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn into_cosmos_msg(self, contract_addr: &HumanAddr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from(contract_addr),
            msg: to_binary(&HandleMsg::Callback(self))?,
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
    /// Amount of assets not locked in TerrsSwap pool; pending refund or liquidation
    pub unlocked_assets: [Asset; 2],
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
