use cosmwasm_std::{Binary, Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// Receive interface for send token.
    Receive(Cw20ReceiveMsg),
    /// Set the owener
    UpdateConfig {
        owner: Option<HumanAddr>,
        reward_contract: Option<HumanAddr>,
        token_contract: Option<HumanAddr>,
        airdrop_registry_contract: Option<HumanAddr>,
    },
    /// Register receives the reward contract address
    RegisterValidator {
        validator: HumanAddr,
    },
    // Remove the validator from validators whitelist
    DeregisterValidator {
        validator: HumanAddr,
    },
    /// update the parameters that is needed for the contract
    UpdateParams {
        epoch_period: Option<u64>,
        unbonding_period: Option<u64>,
        peg_recovery_fee: Option<Decimal>,
        er_threshold: Option<Decimal>,
    },
    /// Receives the underlying coin denom, issues bAsset
    Bond {
        validator: HumanAddr,
    },
    /// Update global index
    UpdateGlobalIndex {
        airdrop_hooks: Option<Vec<Binary>>,
    },
    /// Send back unbonded coin to the user
    WithdrawUnbonded {},
    /// Check whether the slashing has happened or not
    CheckSlashing {},
    /// Claim airdrop
    ClaimAirdrop {
        airdrop_token_contract: HumanAddr,
        airdrop_contract: HumanAddr,
        airdrop_swap_contract: HumanAddr,
        claim_msg: Binary,
        swap_msg: Binary,
    },
    /// Swaps claimed airdrop tokens to UST through Terraswap & sends resulting UST to bLuna reward contract
    SwapHook {
        airdrop_token_contract: HumanAddr,
        airdrop_swap_contract: HumanAddr,
        swap_msg: Binary,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    State {},
    Parameters {},
    CurrentBatch {},
    WhitelistedValidators {},
    WithdrawableUnbonded {
        address: HumanAddr,
        block_time: u64,
    },
    UnbondRequests {
        address: HumanAddr,
    },
    AllHistory {
        start_from: Option<u64>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ParametersResponse {
    pub epoch_period: u64,
    pub underlying_coin_denom: String,
    pub unbonding_period: u64,
    pub peg_recovery_fee: Decimal,
    pub er_threshold: Decimal,
    pub reward_denom: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    pub exchange_rate: Decimal,
    pub total_bond_amount: Uint128,
    pub last_index_modification: u64,
    pub prev_hub_balance: Uint128,
    pub actual_unbonded_amount: Uint128,
    pub last_unbonded_time: u64,
    pub last_processed_batch: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CurrentBatchResponse {
    pub id: u64,
    pub requested_with_fee: Uint128,
}
