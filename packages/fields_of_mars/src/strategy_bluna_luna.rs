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
    /// Address of the protocol treasury to receive fees payments
    pub treasury: HumanAddr,
    /// Address of bLUNA hub contract
    pub bluna_hub: HumanAddr,
    /// Address of the bLUNA token
    pub bluna_token: HumanAddr,
    /// Address of the validator to use when bonding LUNA
    pub bluna_validator: HumanAddr,
    /// Address of Terraswap bLUNA-LUNA pair
    pub pool: HumanAddr,
    /// Address of Terraswap LP token
    pub pool_token: HumanAddr,
    /// Address of Mars liquidity pool
    pub mars: HumanAddr,
    /// Percentage of asset to be charged as liquidation fee
    pub liquidation_fee_rate: Decimal,
    /// Maximum utilization above which a user can be liquidated
    pub liquidation_threshold: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Open a new position or add to an existing position
    IncreasePosition {
        bluna_amount: Uint128,
    },
    /// Reduce a position, or close it completely
    ReducePosition {
        pool_units: Option<Uint128>,
    },
    /// Pay down debt owed to Mars, reduce debt units
    PayDebt {
        user: Option<HumanAddr>,
    },
    /// Close an underfunded position, pay down remaining debt and claim the collateral
    Liquidate {
        user: HumanAddr,
    },
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
    /// Provide specified amounts of bLUNA and LUNA to the Terraswap pool, receive LP tokens
    ProvideLiquidity {
        user: HumanAddr,
        luna_amount: Uint128,
        bluna_amount: Uint128,
    },
    /// Burn LP tokens, remove the liquidity from Terraswap, receive bLUNA and LUNA
    RemoveLiquidity {
        user: HumanAddr,
        pool_units: Uint128,
    },
    /// Borrow LUNA as uncollateralized loan from Mars
    Borrow {
        user: HumanAddr,
        borrow_amount: Uint128,
    },
    /// Pay specified amount of LUNA to Mars
    Repay {
        user: HumanAddr,
        repay_amount: Uint128,
    },
    /// Verify the user's debt ratio, then refund unstaked token and UST to the user
    Refund {
        user: HumanAddr,
    },
    ClaimCollateral {
        user: HumanAddr,
        liquidator: HumanAddr,
        repay_amount: Uint128,
    },
    UpdateConfig {
        new_config: InitMsg,
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
    /// Value of LP tokens owned by the strategy, measured in LUNA
    pub total_pool_value: Uint128,
    /// Amount of pool units; each unit represents a share of the LP tokens owned by the strategy
    pub total_pool_units: Uint128,
    /// Value of debt owed by the strategy to Mars, measured in LUNA
    pub total_debt_value: Uint128,
    /// Amount of debt units; each unit represents a share of the debt owed by the strategy to Mars
    pub total_debt_units: Uint128,
    /// The strategy's overall loan-to-value ratio
    pub utilization: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PositionResponse {
    /// Whether the position is actively farming, or closed pending liquidation
    pub is_active: bool,
    /// Value of the LP tokens assigned to the user
    pub pool_value: Uint128,
    /// Amount of pool units assigned to the user
    pub pool_units: Uint128,
    /// Value of the debt assigned to the user
    pub debt_value: Uint128,
    /// Amount of debt units assigned to the user
    pub debt_units: Uint128,
    /// The user's loan-to-value ratio
    pub utilization: Option<Decimal>,
    /// Amount of LUNA not locked in Terraswap; pending refund or liquidation
    pub unlocked_luna_amount: Uint128,
    /// Amount of bLUNA not locked in Terraswap; pending refund or liquidation
    pub unlocked_bluna_amount: Uint128,
}
