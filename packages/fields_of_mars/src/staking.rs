use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, CosmosMsg, Decimal, Extern, HumanAddr, Querier,
    QueryRequest, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

pub mod anchor_staking {
    use super::{
        Cw20ReceiveMsg, Decimal, Deserialize, HumanAddr, JsonSchema, Serialize, Uint128,
    };

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MockInitMsg {
        /// Address of ANC token
        pub anchor_token: HumanAddr,
        /// Address of ANC-UST LP token
        pub staking_token: HumanAddr,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum HandleMsg {
        /// Receive ANC-UST LP tokens
        Receive(Cw20ReceiveMsg),
        /// Withdraw ANC-UST LP tokens
        Unbond {
            amount: Uint128,
        },
        /// Withdraw pending rewards
        Withdraw {},
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Cw20HookMsg {
        Bond {},
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        StakerInfo {
            staker: HumanAddr,
            block_height: Option<u64>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct StakerInfoResponse {
        pub staker: HumanAddr,
        pub reward_index: Decimal,
        pub bond_amount: Uint128,
        pub pending_reward: Uint128,
    }
}

pub mod mirror_staking {
    use super::{Cw20ReceiveMsg, Deserialize, HumanAddr, JsonSchema, Serialize, Uint128};

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MockInitMsg {
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
        /// Receive MIR-UST LP tokens
        Receive(Cw20ReceiveMsg),
        /// Withdraw MIR-UST LP tokens
        Unbond {
            asset_token: HumanAddr,
            amount: Uint128,
        },
        /// Withdraw pending rewards
        Withdraw {
            asset_token: Option<HumanAddr>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Cw20HookMsg {
        Bond {
            asset_token: HumanAddr,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
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
}

//----------------------------------------------------------------------------------------
// Adapter
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingConfig {
    /// Address of the staking contract
    pub contract_addr: HumanAddr,
    /// Address of the asset token (MIR, mAsset, ANC)
    pub asset_token: HumanAddr,
    /// Address of the token that is to be bonded (typically, a TerraSwap LP token)
    pub staking_token: HumanAddr,
    /// Address of the token to be claimed as staking reward
    pub reward_token: HumanAddr,
}

impl StakingConfig {
    /// @notice Convert `StakingConfig` to `StakingConfigRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<StakingConfigRaw> {
        Ok(StakingConfigRaw {
            contract_addr: deps.api.canonical_address(&self.contract_addr)?,
            asset_token: deps.api.canonical_address(&self.asset_token)?,
            staking_token: deps.api.canonical_address(&self.staking_token)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingConfigRaw {
    /// Address of the staking contract
    pub contract_addr: CanonicalAddr,
    /// Address of the asset token (MIR, mAsset, ANC)
    pub asset_token: CanonicalAddr,
    /// Address of the token that is to be bonded (typically, a TerraSwap LP token)
    pub staking_token: CanonicalAddr,
}

impl StakingConfigRaw {
    /// @notice Convert `StakingConfigRaw` to `StakingConfig`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<StakingConfig> {
        Ok(StakingConfig {
            contract_addr: deps.api.human_address(&self.contract_addr)?,
            asset_token: deps.api.human_address(&self.asset_token)?,
            staking_token: deps.api.human_address(&self.staking_token)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Staking {
    /// Anchor staking contract
    Anchor(StakingConfig),
    /// Mirror V2 staking contract
    Mirror(StakingConfig),
}

impl Staking {
    /// @notice Extract the `StakingConfig` object
    pub fn get_config(&self) -> StakingConfig {
        match &self {
            Self::Anchor(config) => config.clone(),
            Self::Mirror(config) => config.clone(),
        }
    }

    /// @notice Convert `Staking` to `StakingRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<StakingRaw> {
        match &self {
            Self::Anchor(config) => Ok(StakingRaw::Anchor(config.to_raw(deps)?)),
            Self::Mirror(config) => Ok(StakingRaw::Mirror(config.to_raw(deps)?)),
        }
    }

    /// @notice Return the amount of LP tokens bonded to the staking contract
    pub fn query_bond<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<Uint128> {
        let RewardInfoParsed {
            bond_amount,
            ..
        } = self._query_reward_info(deps, staker)?;
        Ok(bond_amount)
    }

    /// @notice Return the amount of claimable reward
    pub fn query_reward<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<Uint128> {
        let RewardInfoParsed {
            pending_reward,
            ..
        } = self._query_reward_info(deps, staker)?;
        Ok(pending_reward)
    }

    /// @notice Generate a message for bonding LP tokens
    pub fn bond_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match &self {
            Self::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: config.contract_addr.clone(),
                    amount,
                    msg: Some(to_binary(&anchor_staking::Cw20HookMsg::Bond {})?),
                })?,
            })),
            Self::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: config.contract_addr.clone(),
                    amount,
                    msg: Some(to_binary(&mirror_staking::Cw20HookMsg::Bond {
                        asset_token: config.asset_token.clone(),
                    })?),
                })?,
            })),
        }
    }

    /// @notice Generate a message for unbonding LP tokens
    pub fn unbond_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match &self {
            Self::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                send: vec![],
                msg: to_binary(&anchor_staking::HandleMsg::Unbond {
                    amount,
                })?,
            })),
            Self::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                send: vec![],
                msg: to_binary(&mirror_staking::HandleMsg::Unbond {
                    asset_token: config.asset_token.clone(),
                    amount,
                })?,
            })),
        }
    }

    /// @notice Generate a message for claiming staking rewards
    pub fn withdraw_message(&self) -> StdResult<CosmosMsg> {
        match &self {
            Self::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                send: vec![],
                msg: to_binary(&anchor_staking::HandleMsg::Withdraw {})?,
            })),
            Self::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                send: vec![],
                msg: to_binary(&mirror_staking::HandleMsg::Withdraw {
                    asset_token: Some(config.asset_token.clone()),
                })?,
            })),
        }
    }

    /// @notice Return the amounts of 1) bonded `staking_tokens` and 2) claimable reward
    fn _query_reward_info<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<RewardInfoParsed> {
        match &self {
            Self::Anchor(config) => {
                let response: anchor_staking::StakerInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.clone(),
                        msg: to_binary(&anchor_staking::QueryMsg::StakerInfo {
                            staker: HumanAddr::from(staker),
                            block_height: None,
                        })?,
                    }))?;
                Ok(RewardInfoParsed {
                    bond_amount: response.bond_amount,
                    pending_reward: response.pending_reward,
                })
            }
            Self::Mirror(config) => {
                let response: mirror_staking::RewardInfoResponse =
                    deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.clone(),
                        msg: to_binary(&mirror_staking::QueryMsg::RewardInfo {
                            staker_addr: HumanAddr::from(staker),
                            asset_token: Some(config.asset_token.clone()),
                        })?,
                    }))?;
                Ok(if response.reward_infos.len() > 0 {
                    RewardInfoParsed {
                        bond_amount: response.reward_infos[0].bond_amount,
                        pending_reward: response.reward_infos[0].pending_reward,
                    }
                } else {
                    RewardInfoParsed::zero()
                })
            }
        }
    }
}

//----------------------------------------------------------------------------------------
// Raw Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum StakingRaw {
    /// Anchor staking contract
    Anchor(StakingConfigRaw),
    /// Mirror V2 staking contract
    Mirror(StakingConfigRaw),
}

impl StakingRaw {
    /// @notice Convert `StakingRaw` to `Staking`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Staking> {
        match &self {
            Self::Anchor(config) => Ok(Staking::Anchor(config.to_normal(deps)?)),
            Self::Mirror(config) => Ok(Staking::Mirror(config.to_normal(deps)?)),
        }
    }
}

pub struct RewardInfoParsed {
    /// Amount of `staking_token` currently bonded in the staking contract
    pub bond_amount: Uint128,
    /// Amount of claimable reward
    pub pending_reward: Uint128,
}

impl RewardInfoParsed {
    pub const fn zero() -> Self {
        Self {
            bond_amount: Uint128::zero(),
            pending_reward: Uint128::zero(),
        }
    }
}
