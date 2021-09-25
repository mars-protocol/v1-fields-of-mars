use cosmwasm_std::{
    to_binary, Addr, Api, CosmosMsg, Decimal, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingConfigBase<T> {
    /// Address of the staking contract
    pub contract_addr: T,
    /// Address of the asset token (MIR, mAsset, ANC)
    pub asset_token: T,
    /// Address of the token that is to be bonded (ANC-UST, MIR-UST, or mAsset-UST LP tokens)
    pub staking_token: T,
}

pub type StakingConfigUnchecked = StakingConfigBase<String>;
pub type StakingConfig = StakingConfigBase<Addr>;

impl From<StakingConfig> for StakingConfigUnchecked {
    fn from(config: StakingConfig) -> Self {
        StakingConfigUnchecked {
            contract_addr: config.contract_addr.to_string(),
            asset_token: config.asset_token.to_string(),
            staking_token: config.staking_token.to_string(),
        }
    }
}

impl StakingConfig {
    pub fn from_unchecked(
        api: &dyn Api,
        config_unchecked: StakingConfigUnchecked,
    ) -> StdResult<Self> {
        Ok(StakingConfig {
            contract_addr: api.addr_validate(&config_unchecked.contract_addr)?,
            asset_token: api.addr_validate(&config_unchecked.asset_token)?,
            staking_token: api.addr_validate(&config_unchecked.staking_token)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingBase<T> {
    /// Anchor staking contract, or those forked from it, e.g. Pylon and Mars
    Anchor(T),
    /// Mirror V2 staking contract
    Mirror(T),
}

pub type StakingUnchecked = StakingBase<StakingConfigUnchecked>;
pub type Staking = StakingBase<StakingConfig>;

impl From<Staking> for StakingUnchecked {
    fn from(staking: Staking) -> Self {
        match staking {
            Staking::Anchor(config) => StakingUnchecked::Anchor(config.into()),
            Staking::Mirror(config) => StakingUnchecked::Mirror(config.into()),
        }
    }
}

impl Staking {
    pub fn from_unchecked(api: &dyn Api, staking_unchecked: StakingUnchecked) -> StdResult<Self> {
        Ok(match staking_unchecked {
            StakingUnchecked::Anchor(config_unchecked) => {
                Staking::Anchor(StakingConfig::from_unchecked(api, config_unchecked)?)
            }
            StakingUnchecked::Mirror(config_unchecked) => {
                Staking::Mirror(StakingConfig::from_unchecked(api, config_unchecked)?)
            }
        })
    }

    /// Query the amount of LP tokens bonded to the staking contract
    pub fn query_bond(&self, querier: &QuerierWrapper, staker: &Addr) -> StdResult<Uint128> {
        let (bond_amount, _) = self._query_reward_info(querier, staker)?;
        Ok(bond_amount)
    }

    /// Query the amount of claimable reward
    pub fn query_reward(&self, querier: &QuerierWrapper, staker: &Addr) -> StdResult<Uint128> {
        let (_, pending_reward) = self._query_reward_info(querier, staker)?;
        Ok(pending_reward)
    }

    /// Generate a message for bonding LP tokens
    pub fn bond_msg(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match self {
            Staking::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.contract_addr.to_string(),
                    amount,
                    msg: to_binary(&anchor_msg::Cw20HookMsg::Bond {})?,
                })?,
                funds: vec![],
            })),

            Staking::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.contract_addr.to_string(),
                    amount,
                    msg: to_binary(&mirror_msg::Cw20HookMsg::Bond {
                        asset_token: config.asset_token.to_string(),
                    })?,
                })?,
                funds: vec![],
            })),
        }
    }

    /// Generate a message for unbonding LP tokens
    pub fn unbond_msg(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match self {
            Staking::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.to_string(),
                msg: to_binary(&anchor_msg::ExecuteMsg::Unbond { amount })?,
                funds: vec![],
            })),

            Staking::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.to_string(),
                msg: to_binary(&mirror_msg::ExecuteMsg::Unbond {
                    asset_token: config.asset_token.to_string(),
                    amount,
                })?,
                funds: vec![],
            })),
        }
    }

    /// Generate a message for claiming staking rewards
    pub fn withdraw_msg(&self) -> StdResult<CosmosMsg> {
        match self {
            Staking::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.to_string(),
                msg: to_binary(&anchor_msg::ExecuteMsg::Withdraw {})?,
                funds: vec![],
            })),

            Staking::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.contract_addr.to_string(),
                msg: to_binary(&mirror_msg::ExecuteMsg::Withdraw {
                    asset_token: Some(config.asset_token.to_string()),
                })?,
                funds: vec![],
            })),
        }
    }

    /// Return the amounts of 1) bonded `staking_tokens` and 2) claimable reward
    fn _query_reward_info(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
    ) -> StdResult<(Uint128, Uint128)> {
        match self {
            Staking::Anchor(config) => {
                let response: anchor_msg::StakerInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&anchor_msg::QueryMsg::StakerInfo {
                            staker: staker.to_string(),
                        })?,
                    }))?;
                Ok((response.bond_amount, response.pending_reward))
            }

            Staking::Mirror(config) => {
                let response: mirror_msg::RewardInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&mirror_msg::QueryMsg::RewardInfo {
                            staker_addr: staker.to_string(),
                            asset_token: Some(config.asset_token.to_string()),
                        })?,
                    }))?;

                if response.reward_infos.is_empty() {
                    Ok((Uint128::zero(), Uint128::zero()))
                } else {
                    let reward_info = &response.reward_infos[0];
                    Ok((reward_info.bond_amount, reward_info.pending_reward))
                }
            }
        }
    }
}

pub mod anchor_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        Receive(Cw20ReceiveMsg),
        Unbond { amount: Uint128 },
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
        /// Anchor staking uses optional parameter `block_height`, while Mars uses `block_timestamp`.
        /// Here we simply omits both so that this message type can work with both variants
        StakerInfo { staker: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct StakerInfoResponse {
        pub staker: String,
        pub reward_index: Decimal,
        pub bond_amount: Uint128,
        pub pending_reward: Uint128,
    }
}

pub mod anchor_mock_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct InstantiateMsg {
        pub anchor_token: String,
        pub staking_token: String,
    }
}

pub mod mirror_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        Receive(Cw20ReceiveMsg),
        Unbond {
            asset_token: String,
            amount: Uint128,
        },
        Withdraw {
            asset_token: Option<String>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Cw20HookMsg {
        Bond { asset_token: String },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        RewardInfo {
            staker_addr: String,
            asset_token: Option<String>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct RewardInfoResponse {
        pub staker_addr: String,
        pub reward_infos: Vec<RewardInfoResponseItem>,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct RewardInfoResponseItem {
        pub asset_token: String,
        pub bond_amount: Uint128,
        pub pending_reward: Uint128,
        pub is_short: bool,
    }
}

pub mod mirror_mock_msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MockInstantiateMsg {
        pub mirror_token: String,
        pub asset_token: String,
        pub staking_token: String,
    }
}
