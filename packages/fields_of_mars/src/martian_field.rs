use cosmwasm_std::{to_binary, Addr, Api, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{AssetBase, AssetInfoBase, OracleBase, PairBase, RedBankBase, StakingBase};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigBase<T> {
    /// Info of the primary asset
    ///
    /// Primary asset is the token that staking reward is paid in. By utilizing Martian Field, the
    /// user takes an implicit long position on the primary asset.
    ///
    /// E.g. In ANC-UST LP strategy, ANC is the primary asset.
    pub primary_asset_info: AssetInfoBase<T>,
    /// Info of the secondary asset
    ///
    /// Secondary asset is the token to be borrowed from Red Bank. By utilizing Martian Field, the
    /// user takes an implicit short position on the secondary asset.
    ///
    /// E.g. In ANC-UST LP strategy, UST is the secondary asset.
    pub secondary_asset_info: AssetInfoBase<T>,
    /// Mars money market aka Red Bank
    pub red_bank: RedBankBase<T>,
    /// Mars oracle contract
    pub oracle: OracleBase<T>,
    /// Astroport pair of primary/secondary assets
    pub pair: PairBase<T>,
    /// Staking contract where share tokens can be bonded to earn rewards
    ///
    /// NOTE: It is assumed that the token to be staked is `pair.share_token`, and the reward in paid
    /// in the primary asset.
    pub staking: StakingBase<T>,
    /// Account to receive fee payments
    pub treasury: T,
    /// Account who can update config
    pub governance: T,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub fee_rate: Decimal,
    /// During liquidation, percentage of the user's asset to be awared to the liquidator as bonus
    pub bonus_rate: Decimal,
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
            bonus_rate: config.bonus_rate,
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
            bonus_rate: self.bonus_rate,
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

    use crate::adapters::AssetUnchecked;

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
        ///
        /// Deposit primary asset and optionally secondary asset that is no more in value than the
        /// primary asset deposit. The contract will compute the market value of the assets, and borrow
        /// secondary asset from Red Bank to make the two even in value.
        IncreasePosition {
            deposits: Vec<AssetUnchecked>,
        },
        /// Reduce a position, or close it completely
        ///
        /// Liquidity are withdrawn from the AMM; primary asset of `swap_amount` is swapped for the
        /// secondary asset; then, secondary asset of `repay_amount` is repaid to Red Bank. If the user's
        /// LTV is no greater than `max_ltv` after these actions are completed, the remaining withdrawn
        /// assets are refunded to the user.
        ///
        /// NOTE: `repay_amount` is the actual amount to be delivered to Red Bank. Due to tax, tf the
        /// secondary asset is a native token, an amount slightly greater than `repay_amount` needs
        /// to be available in the user's position after performing the swap.
        ReducePosition {
            bond_units: Uint128,
            swap_amount: Uint128,
            repay_amount: Uint128,
        },
        /// Pay down debt owed to Mars, reduce debt units
        ///
        /// NOTE: `repay_amount` is the actual amount to be delivered to Red Bank. Due to tax, if the
        /// secondary asset is a native token, an amount slightly greater than `repay_amount` needs
        /// to be deposited. The excess amount will be refunded to the user.
        ///
        /// E.g. Suppose the tax associated with transferring 100 UST is 0.1 UST. To reduce the user's
        /// debt by 100 UST, set `repay_amount` as 100.1e6 and transfer at least 1_001_000 uusd with
        /// the message.
        PayDebt {
            repay_amount: Uint128,
        },
        /// Close an underfunded position
        ///
        /// Liquidity are withdrawn from the AMM; all primary assets are swapped for the secondary
        /// asset; debt is fully paid with the proceedings. Among the remaining secondary assets, a
        /// portion corresponding to `bonus_rate` is awarded to the liquidator, while the rest is
        /// refunded to the user.
        Liquidate {
            user: String,
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
        /// Provide unlocked primary & secondary assets to the AMM pool, receive share tokens;
        /// Reduce the user's unlocked primary & secondary asset amounts to zero;
        /// Increase the user's unlocked share token amount
        ProvideLiquidity {
            user_addr: Addr,
        },
        /// Burn the user's unlocked share tokens, receive primary & secondary assets;
        /// Reduce the user's unlocked share token amount to zero;
        /// Increase the user's unlocked primary & secondary asset amounts
        WithdrawLiquidity {
            user_addr: Addr,
        },
        /// Bond share tokens to the staking contract;
        /// Reduce the user's unlocked share token amount to zero;
        /// Increase the user's bond units
        Bond {
            user_addr: Addr,
        },
        /// Unbond share tokens from the staking contract;
        /// Reduce the user's bond units;
        /// Increase the user's unlocked share token amount
        Unbond {
            user_addr: Addr,
            bond_units: Uint128,
        },
        /// Borrow specified amount of secondary asset from Red Bank;
        /// Increase the user's debt units;
        /// Increase the user's unlocked secondary asset amount
        Borrow {
            user_addr: Addr,
            borrow_amount: Uint128,
        },
        /// Repay specified amount of secondary asset to Red Bank;
        /// Reduce the user's debt units;
        /// Reduce the user's unlocked secondary asset amount
        Repay {
            user_addr: Addr,
            repay_amount: Uint128,
        },
        /// Swap a specified amount of primary asset to secondary asset;
        /// Reduce the user's unlocked primary asset amount;
        /// Increase the user's unlocked secondary asset amount;
        ///
        /// If `swap_amount` is not provided, then use all available unlocked primary asset
        Swap {
            user_addr: Addr,
            swap_amount: Option<Uint128>,
        },
        /// Send a percentage of a user's unlocked primary & seoncdary asset to the specified recipient;
        /// Reduce the user's primary & secondary asset amound
        Refund {
            user_addr: Addr,
            recipient: Addr,
            percentage: Decimal,
        },
        /// Calculate a user's current LTV; throw error if it is above the maximum LTV specified in config
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
