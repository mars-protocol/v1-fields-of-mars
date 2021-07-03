use cosmwasm_std::{Decimal, HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use terraswap::asset::Asset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Address of MIR token
    pub mirror_token: HumanAddr,
    /// Address of the token to be staked (MIR or mAsset)
    pub asset_token: HumanAddr,
    /// Address of MIR-UST LP token
    pub staking_token: HumanAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<HumanAddr>,
        premium_min_update_interval: Option<u64>,
    },
    RegisterAsset {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
    },
    Unbond {
        asset_token: HumanAddr,
        amount: Uint128,
    },
    Withdraw {
        asset_token: Option<HumanAddr>,
    },
    AutoStake {
        assets: [Asset; 2],
        slippage_tolerance: Option<Decimal>,
    },
    AutoStakeHook {
        asset_token: HumanAddr,
        staking_token: HumanAddr,
        staker_addr: HumanAddr,
        prev_staking_token_amount: Uint128,
    },
    AdjustPremium {
        asset_tokens: Vec<HumanAddr>,
    },
    IncreaseShortToken {
        asset_token: HumanAddr,
        staker_addr: HumanAddr,
        amount: Uint128,
    },
    DecreaseShortToken {
        asset_token: HumanAddr,
        staker_addr: HumanAddr,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Bond {
        asset_token: HumanAddr,
    },
    DepositReward {
        rewards: Vec<(HumanAddr, Uint128)>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    PoolInfo {
        asset_token: HumanAddr,
    },
    RewardInfo {
        staker_addr: HumanAddr,
        asset_token: Option<HumanAddr>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponse {
    pub staker_addr: HumanAddr,
    pub reward_infos: Vec<RewardInfoResponseItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfoResponseItem {
    pub asset_token: HumanAddr,
    pub bond_amount: Uint128,
    pub pending_reward: Uint128,
    pub is_short: bool,
}
