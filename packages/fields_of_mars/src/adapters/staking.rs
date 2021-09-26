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
    contract_addr: T,
    /// Address of ANC, MIR, or mAsset token; refer to Mirror contract for definition
    asset_token: T,
    /// Address of ANC-UST, MIR-UST, or mAsset-UST LP token
    staking_token: T,
}

pub type StakingConfigUnchecked = StakingConfigBase<String>;
pub type StakingConfig = StakingConfigBase<Addr>;

impl From<StakingConfig> for StakingConfigUnchecked {
    fn from(checked: StakingConfig) -> Self {
        StakingConfigUnchecked {
            contract_addr: checked.contract_addr.to_string(),
            asset_token: checked.asset_token.to_string(),
            staking_token: checked.staking_token.to_string(),
        }
    }
}

impl StakingConfigUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<StakingConfig> {
        let checked = StakingConfig {
            contract_addr: api.addr_validate(&self.contract_addr)?,
            asset_token: api.addr_validate(&self.asset_token)?,
            staking_token: api.addr_validate(&self.staking_token)?,
        };

        Ok(checked)
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
    fn from(checked: Staking) -> Self {
        match checked {
            Staking::Anchor(config) => StakingUnchecked::Anchor(config.into()),
            Staking::Mirror(config) => StakingUnchecked::Mirror(config.into()),
        }
    }
}

impl StakingUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Staking> {
        let checked = match self {
            StakingUnchecked::Anchor(config) => Staking::Anchor(config.check(api)?),
            StakingUnchecked::Mirror(config) => Staking::Mirror(config.check(api)?),
        };

        Ok(checked)
    }
}

impl Staking {
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
    pub fn query_reward_info(
        &self,
        querier: &QuerierWrapper,
        staker_addr: &Addr,
    ) -> StdResult<(Uint128, Uint128)> {
        let (bonded_amount, pending_reward_amount) = match self {
            Staking::Anchor(config) => {
                let response: anchor_msg::StakerInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&anchor_msg::QueryMsg::StakerInfo {
                            staker: staker_addr.to_string(),
                        })?,
                    }))?;

                (response.bond_amount, response.pending_reward)
            }

            Staking::Mirror(config) => {
                let response: mirror_msg::RewardInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&mirror_msg::QueryMsg::RewardInfo {
                            staker_addr: staker_addr.to_string(),
                            asset_token: Some(config.asset_token.to_string()),
                        })?,
                    }))?;

                if response.reward_infos.is_empty() {
                    (Uint128::zero(), Uint128::zero())
                } else {
                    let reward_info = &response.reward_infos[0];
                    (reward_info.bond_amount, reward_info.pending_reward)
                }
            }
        };

        Ok((bonded_amount, pending_reward_amount))
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
        pub asset_and_staking_tokens: Vec<(String, String)>,
    }
}
