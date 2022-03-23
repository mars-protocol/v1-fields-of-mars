use std::str::FromStr;

use cosmwasm_std::{
    to_binary, Addr, Api, CosmosMsg, Decimal, StdError, StdResult, Uint128, WasmMsg, Empty
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_asset::{AssetInfoBase, AssetUnchecked, AssetInfo, AssetListUnchecked};

use crate::adapters::{GeneratorBase, OracleBase, PairBase, RedBankBase};

const MIN_MAX_LTV: &str = "0.55";
const MAX_MAX_LTV: &str = "0.75";
const MAX_FEE_RATE: &str = "0.1";
const MAX_BONUS_RATE: &str = "0.1";

//--------------------------------------------------------------------------------------------------
// Config
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigBase<T> {
    /// Info of the primary asset
    ///
    /// Primary asset is the asset which the user takes an implicit long position on when utilizing
    /// Martian Field. Taking the ANC-UST strategy for example; if the user primarily deposits ANC
    /// and borrows UST from Red Bank, then ANC is the primary asset.
    pub primary_asset_info: AssetInfoBase<T>,
    /// Info of the secondary asset
    ///
    /// Secondary asset is the asset which the user takes an implicit short position on when utilizing
    /// Martian Field. Taking the ANC-UST strategy for example; if the user primarily deposits ANC
    /// and borrows UST from Red Bank, then UST is the secondary asset.
    pub secondary_asset_info: AssetInfoBase<T>,
    /// Info of the Astroport token, the staking reward that will be paid out by Astro generator
    ///
    /// Astro generator may also pay out a "proxy reward", e.g. ANC for the ANC-UST strategy. Here
    /// we make the assumption that this proxy reward is always the primary asset. Note that we do
    /// not assert this when instantiating the contract, so it is the deployer's responsibility to
    /// make sure of this.
    pub astro_token_info: AssetInfoBase<T>,
    /// Astroport pair consisting of the primary and secondary assets
    ///
    /// The liquidity token of this pair will be staked/bonded in Astro generator to earn ASTRO and
    /// optionally a proxy token reward.
    pub primary_pair: PairBase<T>,
    /// Astroport pair consisting of ASTRO token and the secondary asset
    ///
    /// This pair is used for swapping ASTRO reward so that it can be reinvested.
    pub astro_pair: PairBase<T>,
    /// The Astro generator contract
    pub astro_generator: GeneratorBase<T>,
    /// The Mars Protocol money market contract. We borrow the secondary asset here
    pub red_bank: RedBankBase<T>,
    /// The Mars Protocol oracle contract. We read prices of the primary and secondary assets here
    pub oracle: OracleBase<T>,
    /// Account to receive fee payments
    pub treasury: T,
    /// Account who can update config
    pub governance: T,
    /// Accounts who can harvest
    pub operators: Vec<T>,
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
            astro_token_info: config.astro_token_info.into(),
            primary_pair: config.primary_pair.into(),
            astro_pair: config.astro_pair.into(),
            astro_generator: config.astro_generator.into(),
            red_bank: config.red_bank.into(),
            oracle: config.oracle.into(),
            treasury: config.treasury.into(),
            governance: config.governance.into(),
            operators: config.operators.iter().map(|op| op.to_string()).collect(),
            max_ltv: config.max_ltv,
            fee_rate: config.fee_rate,
            bonus_rate: config.bonus_rate,
        }
    }
}

impl ConfigUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Config> {
        Ok(Config {
            primary_asset_info: self.primary_asset_info.check(api, None)?,
            secondary_asset_info: self.secondary_asset_info.check(api, None)?,
            astro_token_info: self.astro_token_info.check(api, None)?,
            primary_pair: self.primary_pair.check(api)?,
            astro_pair: self.astro_pair.check(api)?,
            astro_generator: self.astro_generator.check(api)?,
            red_bank: self.red_bank.check(api)?,
            oracle: self.oracle.check(api)?,
            treasury: api.addr_validate(&self.treasury)?,
            governance: api.addr_validate(&self.governance)?,
            operators: self
                .operators
                .iter()
                .map(|op| api.addr_validate(op))
                .collect::<StdResult<Vec<Addr>>>()?,
            max_ltv: self.max_ltv,
            fee_rate: self.fee_rate,
            bonus_rate: self.bonus_rate,
        })
    }
}

impl Config {
    pub fn validate(&self) -> StdResult<()> {
        let min_max_ltv = Decimal::from_str(MIN_MAX_LTV)?;
        let max_max_ltv = Decimal::from_str(MAX_MAX_LTV)?;
        if self.max_ltv < min_max_ltv || self.max_ltv > max_max_ltv {
            return Err(StdError::generic_err(format!(
                "invalid max ltv: {}; must be in [{}, {}]",
                self.max_ltv, MIN_MAX_LTV, MAX_MAX_LTV
            )));
        }

        let max_fee_rate = Decimal::from_str(MAX_FEE_RATE)?;
        if self.fee_rate > max_fee_rate {
            return Err(StdError::generic_err(format!(
                "invalid fee rate: {}; must be <= {}",
                self.fee_rate, MAX_FEE_RATE
            )));
        }

        let max_bonus_rate = Decimal::from_str(MAX_BONUS_RATE)?;
        if self.bonus_rate > max_bonus_rate {
            return Err(StdError::generic_err(format!(
                "invalid bonus rate: {}; must be <= {}",
                self.bonus_rate, MAX_BONUS_RATE
            )));
        }

        Ok(())
    }
}

//--------------------------------------------------------------------------------------------------
// Actions: defines a list of actions that users can perform on their positions
//--------------------------------------------------------------------------------------------------

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
        offer_amount: Uint128,
        max_spread: Option<Decimal>,
    },
}

//--------------------------------------------------------------------------------------------------
// Message types
//--------------------------------------------------------------------------------------------------

pub type InstantiateMsg = ConfigUnchecked;

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
    ///
    /// `max_spread` is used for ASTRO >> secondary swap and balancing operations
    ///
    /// `slippage_tolerance` is used for providing primary + secondary liquidity
    Harvest {
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
    ///
    /// If `repay_amount` is not provided, then use all available unlocked secondary asset
    Repay {
        user_addr: Addr,
        repay_amount: Option<Uint128>,
    },
    /// Swap a specified amount of primary asset to secondary asset;
    /// Reduce the user's unlocked primary asset amount;
    /// Increase the user's unlocked secondary asset amount;
    ///
    /// If `swap_amount` is not provided, then use all available unlocked asset
    Swap {
        user_addr: Option<Addr>,
        offer_asset_info: AssetInfo,
        offer_amount: Option<Uint128>,
        max_spread: Option<Decimal>,
    },
    /// Swap the primary and secondary assets currently held by the contract as pending rewards,
    /// such that the two assets have the same value and can be reinvested
    ///
    /// _Only used during the `Harvest` function call_
    Balance {
        max_spread: Option<Decimal>,
    },
    /// Sell an appropriate amount of a user's unlocked primary asset, such that the user has
    /// enough unlocked secondary asset to fully pay off debt
    ///
    /// _Only used during the `Liquidate` function call_
    Cover {
        user_addr: Addr,
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
    /// Calculate a user's current LTV. If below the maximum LTV, emits a `position_updated`
    /// event; if above the maximum LTV, throw an error
    AssertHealth {
        user_addr: Addr,
    },
    /// Check whether the user still has an outstanding debt. If no, do nothing. If yes, waive
    /// the debt from the user's position, and emit a `bad_debt` event
    ///  
    /// Effectively, the bad debt is shared by all other users. An altrustic person can monitor
    /// the event and repay the same amount of debt at Red Bank on behalf of the Fields contract,
    /// so that other users don't have to share the bad debt
    ClearBadDebt {
        user_addr: Addr,
    },
    /// Remove the user's position from contract storage if it is empty. Invoked at the end of
    /// `update_position` and `liquidate` callback chains
    PurgeStorage {
        user_addr: Addr,
    },
}

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
    /// Return the global state of the strategy. Response: `PositionResponse`
    State {},
    /// Return data on an individual user's position. Response: `PositionResponse`
    Position {
        user: String,
    },
    /// Enumerate all user positions. Response: `Vec<PositionsResponseItem>`
    Positions {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

//--------------------------------------------------------------------------------------------------
// Response types
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionsResponseItem {
    pub user: String,
    pub position: PositionResponse,
}

/// `PositionResponse` is used both to describe an individual position, as well as the overall state
/// of the strategy
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    pub bond_units: Uint128,
    pub bond_amount: Uint128,
    pub bond_value: Uint128,
    pub debt_units: Uint128,
    pub debt_amount: Uint128,
    pub debt_value: Uint128,
    pub ltv: Option<Decimal>,
    pub unlocked_assets: AssetListUnchecked,
}

/// We currently don't need any input parameter for migration
pub type MigrateMsg = Empty;
