use cosmwasm_std::{to_binary, Addr, Api, CosmosMsg, Decimal, StdResult, Uint128, WasmMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{AssetBase, AssetInfoBase, OracleBase, PairBase, RedBankBase, StakingBase};

//--------------------------------------------------------------------------------------------------
// Config
//--------------------------------------------------------------------------------------------------

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
    /// NOTE: It is assumed that the token to be staked is `pair.liquidity_token`, and the reward in paid
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

//--------------------------------------------------------------------------------------------------
// State: global state of the contract
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateBase<T> {
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
    /// Reward tokens that can be reinvested in the next harvest
    pub pending_rewards: Vec<AssetBase<T>>,
}

// `Addr` does not have `Default` implemented, so we can't derive the Default trait
impl<T> Default for StateBase<T> {
    fn default() -> Self {
        StateBase {
            total_bond_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
            pending_rewards: vec![],
        }
    }
}

pub type StateUnchecked = StateBase<String>;
pub type State = StateBase<Addr>;

impl From<State> for StateUnchecked {
    fn from(state: State) -> Self {
        StateUnchecked {
            total_bond_units: state.total_bond_units,
            total_debt_units: state.total_debt_units,
            pending_rewards: state
                .pending_rewards
                .iter()
                .map(|asset| asset.clone().into())
                .collect(),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Position, Health, Snapshot: info of individual users' positions
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionBase<T> {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: Vec<AssetBase<T>>,
}

// `Addr` does not have `Default` implemented, so we can't derive the Default trait
impl<T> Default for PositionBase<T> {
    fn default() -> Self {
        PositionBase {
            bond_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unlocked_assets: vec![],
        }
    }
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

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Health {
    /// Value of the position's asset, measured in the short asset
    pub bond_value: Uint128,
    /// Value of the position's debt, measured in the short asset
    pub debt_value: Uint128,
    /// The ratio of `debt_value` to `bond_value`; None if `bond_value` is zero
    pub ltv: Option<Decimal>,
}

/// Every time the user changes the executes `update_position`, we record a snaphot of the position.
///
/// This snapshot does not actually impact the functioning of this contract in any way, but rather
/// used by the frontend to calculate PnL. Once we have the infrastructure for calculating PnL
/// off-chain available, we will migrate the contract to delete this callback
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Snapshot {
    pub time: u64,
    pub height: u64,
    pub position: PositionUnchecked,
    pub health: Health,
}

//--------------------------------------------------------------------------------------------------
// Message and response types
//--------------------------------------------------------------------------------------------------

pub mod msg {
    use super::*;
    use crate::adapters::AssetUnchecked;
    use cosmwasm_std::Empty;

    pub type InstantiateMsg = ConfigUnchecked;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Action {
        /// Deposit asset of specified type and amount
        ///
        /// If the asset is a native token such as UST, the contract verifies the token greater or
        /// equal in amount has been received with the transaction
        ///
        /// If the asset is a CW20 token, the contract will attempt to draw it from the sender's
        /// wallet. NOTE: sender must have approved spending first
        Deposit(AssetUnchecked),
        /// Borrow secondary asset of specified amount from Red Bank
        Borrow {
            amount: Uint128,
        },
        /// Repay secondary asset of specified amount to Red Bank
        ///
        /// NOTE: sender must make sure the position has sufficient amount of secondary asset
        /// (repay amount + tax), either by depositing or swapping
        Repay {
            amount: Uint128,
        },
        /// Provide all unlocked primary and secondary asset to Astroport pair, and bond the
        /// received liquidity tokens to the staking pool
        ///
        /// NOTE: we provide **all** unlocked assets to the pair. Sender must make sure the unlocked
        /// primary and secondary assets are similar in value, or provide a `slippage_tolerance`
        /// parameter
        Bond {
            slippage_tolerance: Option<Decimal>,
        },
        /// Burn a specified amount bond units, unbond liquidity tokens of corresponding amount from
        /// the staking pool and withdraw liquidity
        Unbond {
            bond_units_to_reduce: Uint128,
        },
        /// Swap a specified amount of unlocked primary asset to the secondary asset
        Swap {
            swap_amount: Uint128,
            belief_price: Option<Decimal>,
            max_spread: Option<Decimal>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    #[allow(clippy::large_enum_variant)]
    pub enum ExecuteMsg {
        /// Update the sender's position by executing a list of actions
        ///
        /// After the actions are executed, the contract executes three more callbacks:
        ///
        /// 1. Refund all unlocked assets to the user.
        ///
        /// 2. Assert the position's LTV is below the liquidation threshold. If not, throw an error
        /// and revert all previous actions
        ///
        /// 3. Delete cached data in storage
        UpdatePosition(Vec<Action>),
        /// Claim staking reward and reinvest
        Harvest {
            belief_price: Option<Decimal>,
            max_spread: Option<Decimal>,
            slippage_tolerance: Option<Decimal>,
        },
        /// Force close an underfunded position, repay all debts, and return all remaining funds to
        /// the position's owner. The liquidator is awarded a portion of the remaining funds.
        Liquidate {
            user: String,
        },
        /// Update data stored in config (only governance can call)
        UpdateConfig {
            new_config: ConfigUnchecked,
        },
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
            user_addr: Option<Addr>,
            slippage_tolerance: Option<Decimal>,
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
            user_addr: Option<Addr>,
        },
        /// Unbond share tokens from the staking contract;
        /// Reduce the user's bond units;
        /// Increase the user's unlocked share token amount
        Unbond {
            user_addr: Addr,
            bond_units_to_reduce: Uint128,
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
        /// If `swap_amount` is not provided, then use all available unlocked asset
        Swap {
            user_addr: Option<Addr>,
            swap_amount: Option<Uint128>,
            belief_price: Option<Decimal>,
            max_spread: Option<Decimal>,
        },
        /// Send a percentage of a user's unlocked primary & seoncdary asset to a recipient; default
        /// to the user if unspecified
        ///
        /// Reduce the user's primary & secondary asset amounts
        Refund {
            user_addr: Addr,
            recipient_addr: Addr,
            percentage: Decimal,
        },
        /// Calculate a user's current LTV; throw error if it is above the maximum LTV
        AssertHealth {
            user_addr: Addr,
        },
        /// See the comment on struct `Snapshot`. This callback should be removed at some pointer
        /// after launch when our tx indexing infrastructure is ready
        Snapshot {
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
        /// Return strategy configurations. Response: `ConfigUnchecked`
        Config {},
        /// Return the global state of the strategy. Response: `StateUnchecked`
        State {},
        /// Return data on an individual user's position. Response: `PositionUnchecked`
        Position {
            user: String,
        },
        /// Query the health of a user's position: value of assets, debts, and LTV. Response: `Health`
        Health {
            user: String,
        },
        /// See the comment on struct `Snapshot`. Response: `Snapshot`
        Snapshot {
            user: String,
        },
    }

    /// We currently don't need any input parameter for migration
    pub type MigrateMsg = Empty;
}
