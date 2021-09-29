use cosmwasm_std::{to_binary, Addr, Api, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{AssetBase, AssetInfoBase, OracleBase, PairBase, RedBankBase, StakingBase};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigBase<T> {
    /// Info of the asset to be deposited by the user
    pub primary_asset_info: AssetInfoBase<T>,
    /// Info of the asset to be either deposited by user or borrowed from Mars
    pub secondary_asset_info: AssetInfoBase<T>,
    /// Mars money market aka Red Bank
    pub red_bank: RedBankBase<T>,
    /// Mars oracle contract
    pub oracle: OracleBase<T>,
    /// Astroport pair of primary/secondary assets
    pub pair: PairBase<T>,
    /// Staking contract where LP tokens can be bonded to earn rewards
    pub staking: StakingBase<T>,
    /// Account to receive fee payments
    pub treasury: T,
    /// Account who can update config
    pub governance: T,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub fee_rate: Decimal,
}

pub type ConfigUnchecked = ConfigBase<String>;
pub type Config = ConfigBase<Addr>;

impl From<Config> for ConfigUnchecked {
    fn from(config: Config) -> Self {
        ConfigUnchecked {
            primary_asset_info: config.primary_asset_info.into(),
            secondary_asset_info: config.secondary_asset_info.into(),
            red_bank: config.red_bank.into(),
            oracle: config.oracle.into(),
            pair: config.pair.into(),
            staking: config.staking.into(),
            treasury: config.treasury.into(),
            governance: config.governance.into(),
            max_ltv: config.max_ltv,
            fee_rate: config.fee_rate,
        }
    }
}

impl ConfigUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Config> {
        Ok(Config {
            primary_asset_info: self.primary_asset_info.check(api)?,
            secondary_asset_info: self.secondary_asset_info.check(api)?,
            red_bank: self.red_bank.check(api)?,
            oracle: self.oracle.check(api)?,
            pair: self.pair.check(api)?,
            staking: self.staking.check(api)?,
            treasury: api.addr_validate(&self.treasury)?,
            governance: api.addr_validate(&self.governance)?,
            max_ltv: self.max_ltv,
            fee_rate: self.fee_rate,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
}

impl Default for State {
    fn default() -> Self {
        State {
            total_bond_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionBase<T> {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: Vec<AssetBase<T>>,
}

pub type PositionUnchecked = PositionBase<String>;
pub type Position = PositionBase<Addr>;

impl From<Position> for PositionUnchecked {
    fn from(position: Position) -> Self {
        PositionUnchecked {
            bond_units: position.bond_units,
            debt_units: position.debt_units,
            unlocked_assets: position
                .unlocked_assets
                .iter()
                .map(|asset| asset.clone().into())
                .collect(),
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self {
            bond_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unlocked_assets: vec![],
        }
    }
}

pub mod msg {
    use super::*;

    use crate::adapters::{Asset, AssetUnchecked};

    pub type InstantiateMsg = ConfigUnchecked;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    #[allow(clippy::large_enum_variant)]
    pub enum ExecuteMsg {
        /// Update data stored in config (governance only)
        UpdateConfig {
            new_config: ConfigUnchecked,
        },
        /// Open a new position or add to an existing position
        IncreasePosition {
            deposits: Vec<AssetUnchecked>,
        },
        /// Reduce a position, or close it completely
        ReducePosition {
            bond_units: Option<Uint128>,
            swap_amount: Uint128,
            repay: bool,
        },
        /// Pay down debt owed to Mars, reduce debt units
        PayDebt {
            user: Option<String>,
            deposit: AssetUnchecked,
        },
        /// Pay down remaining debt of a closed position and be awarded its unlocked assets
        Liquidate {
            user: String,
            deposit: AssetUnchecked,
        },
        /// Claim staking reward and reinvest
        Harvest {},
        /// Callbacks; only callable by the strategy itself.
        Callback(CallbackMsg),
    }

    // NOTE: Since CallbackMsg are always sent by the contract itself, we assume all types are already
    // validated and don't do additional checks. E.g. user addresses are Addr instead of String
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum CallbackMsg {
        /// Provide the user's unlocked primary/secondary assets to the AMM, receive share tokens
        ProvideLiquidity {
            user_addr: Addr,
        },
        /// Burn the user's unlocked share tokens, receive primary/secondary assets
        RemoveLiquidity {
            user_addr: Addr,
        },
        /// Bond share tokens to the staking contract
        Bond {
            user_addr: Addr,
        },
        /// Unbond share tokens from the staking contract
        Unbond {
            user_addr: Addr,
            bond_units: Option<Uint128>,
        },
        /// Borrow specified amount of short asset from Mars
        Borrow {
            user_addr: Addr,
            amount: Uint128,
        },
        /// Use the user's unlocked short asset to repay debt
        Repay {
            user_addr: Addr,
        },
        /// Send a percentage of a user's unlocked assets to a specified recipient
        Refund {
            user_addr: Addr,
            recipient: Addr,
            percentage: Decimal,
        },
        /// Collect a portion of rewards as performance fee, swap half of the rest for UST
        Swap {
            user_addr: Addr,
            offer_asset: Asset,
        },
        /// Check if a user's LTV is below liquidation threshold; throw an error if not
        AssertHealth {
            user_addr: Addr,
        },
    }

    // Modified from
    // https://github.com/CosmWasm/cw-plus/blob/v0.8.0/packages/cw20/src/receiver.rs#L23
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
        Position {
            user: String,
        },
        /// Query the health of a user's position: value of assets, debts, and LTV
        Health {
            user: String,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MigrateMsg {}

    pub type ConfigResponse = ConfigUnchecked;
    pub type StateResponse = State;
    pub type PositionResponse = PositionUnchecked;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct HealthResponse {
        /// Value of the position's asset, measured in the short asset
        pub bond_value: Uint128,
        /// Value of the position's debt, measured in the short asset
        pub debt_value: Uint128,
        /// The ratio of `debt_value` to `bond_value`; None if `bond_value` is zero
        pub ltv: Option<Decimal>,
    }
}
