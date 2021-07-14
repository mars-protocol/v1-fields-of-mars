use cosmwasm_std::{
    to_binary, CosmosMsg, Decimal, HumanAddr, StdResult, Uint128, WasmMsg,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

//----------------------------------------------------------------------------------------
// MESSAGES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Account who can update config
    pub owner: HumanAddr,
    /// Accounts who can harvest
    pub operators: Vec<HumanAddr>,
    /// Address of the protocol treasury to receive fees payments
    pub treasury: HumanAddr,
    /// Address of the token to be deposited by users (MIR, mAsset, ANC)
    pub asset_token: HumanAddr,
    /// Address of the token that is to be harvested as rewards (MIR, ANC)
    pub reward_token: HumanAddr,
    /// Address of the TerraSwap pair
    pub pool: HumanAddr,
    /// Address of the TerraSwap LP token
    pub pool_token: HumanAddr,
    /// Address of Mars liquidity pool
    pub mars: HumanAddr,
    /// Address of the staking contract
    pub staking_contract: HumanAddr,
    /// Type of the staking contract ("anchor" or "mirror")
    pub staking_type: String,
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
    IncreasePosition {
        asset_amount: Uint128,
    },
    /// Reduce a position, or close it completely
    ReducePosition {
        bond_units: Option<Uint128>,
    },
    /// Pay down debt owed to Mars, reduce debt units
    PayDebt {
        user: Option<HumanAddr>,
    },
    /// Close an underfunded position, pay down remaining debt and claim the collateral
    Liquidate {
        user: HumanAddr,
    },
    /// Claim staking reward and reinvest
    Harvest {},
    /// Update data stored in config (owner only)
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
        asset_amount: Uint128,
        ust_amount: Uint128,
        user: Option<HumanAddr>,
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
        borrow_amount: Uint128,
    },
    /// Pay specified amount of UST to Mars
    Repay {
        user: HumanAddr,
        repay_amount: Uint128,
    },
    /// Collect a portion of rewards as performance fee, swap half of the rest for UST
    SwapReward {
        reward_amount: Uint128,
    },
    /// Verify the user's debt ratio, then refund unstaked token and UST to the user
    Refund {
        user: HumanAddr,
    },
    /// Receive UST, pay back debt, and credit the liquidator a share of the collateral
    ClaimCollateral {
        user: HumanAddr,
        liquidator: HumanAddr,
        repay_amount: Uint128,
    },
    /// Update data stored in config
    UpdateConfig {
        new_config: InitMsg,
    },
    /// Save a snapshot of a user's position; useful for the frontend to calculate PnL
    UpdatePositionSnapshot {
        user: HumanAddr,
    },
}

// Modified from
// https://github.com/CosmWasm/cosmwasm-plus/blob/v0.2.3/packages/cw20/src/receiver.rs#L15
impl CallbackMsg {
    pub fn into_cosmos_msg(self, contract_addr: &HumanAddr) -> StdResult<CosmosMsg> {
        let execute = WasmMsg::Execute {
            contract_addr: HumanAddr::from(contract_addr),
            msg: to_binary(&HandleMsg::Callback(self))?,
            send: vec![],
        };
        Ok(execute.into())
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
    /// Snapshot of a user's position the last time the position was increased, decreased,
    /// or when debt was paid. Useful for the frontend to calculate PnL
    PositionSnapshot {
        user: HumanAddr,
    },
}

// Migration is not implemented for the current version
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}

//----------------------------------------------------------------------------------------
// RESPONSES
//----------------------------------------------------------------------------------------

pub type ConfigResponse = InitMsg;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// UST value of all bonded LP tokens
    pub total_bond_value: Uint128,
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// UST value of all debt owed to Mars
    pub total_debt_value: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
    /// The strategy's overall loan-to-value ratio
    pub ltv: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    /// Whether the position is actively farming, or closed pending liquidation
    pub is_active: bool,
    /// UST value of the user's share of bonded LP tokens
    pub bond_value: Uint128,
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// UST value of the user's share of debt owed to Mars
    pub debt_value: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// The user's loan-to-value ratio
    pub ltv: Option<Decimal>,
    /// Amount of unbonded UST pending refund or liquidation
    pub unbonded_ust_amount: Uint128,
    /// Amount of unbonded asset token pending refund or liquidation
    pub unbonded_asset_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionSnapshotResponse {
    /// UNIX timestamp at which the snapshot was taken
    pub time: u64,
    /// Block number at which the snapshot was taken
    pub height: u64,
    /// The snapshot
    pub snapshot: PositionResponse,
}