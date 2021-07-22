use cosmwasm_std::{
    to_binary, Addr, Decimal, QuerierWrapper, QueryRequest, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

pub mod anchor_staking {
    use super::{Cw20ReceiveMsg, Decimal, Deserialize, JsonSchema, Serialize, Uint128};

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MockInstantiateMsg {
        /// Address of ANC token
        pub anchor_token: String,
        /// Address of ANC-UST LP token
        pub staking_token: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
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
            staker: String,
            block_height: Option<u64>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct StakerInfoResponse {
        pub staker: String,
        pub reward_index: Decimal,
        pub bond_amount: Uint128,
        pub pending_reward: Uint128,
    }
}

pub mod mirror_staking {
    use super::{Cw20ReceiveMsg, Deserialize, JsonSchema, Serialize, Uint128};

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct MockInstantiateMsg {
        /// Address of MIR token
        pub mirror_token: String,
        /// Address of the token to be staked (MIR or mAsset)
        pub asset_token: String,
        /// Address of MIR-UST LP token
        pub staking_token: String,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum ExecuteMsg {
        /// Receive MIR-UST LP tokens
        Receive(Cw20ReceiveMsg),
        /// Withdraw MIR-UST LP tokens
        Unbond {
            asset_token: String,
            amount: Uint128,
        },
        /// Withdraw pending rewards
        Withdraw {
            asset_token: Option<String>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Cw20HookMsg {
        Bond {
            asset_token: String,
        },
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

//----------------------------------------------------------------------------------------
// Adapter
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingConfig {
    /// Address of the staking contract
    pub contract_addr: String,
    /// Address of the asset token (MIR, mAsset, ANC)
    pub asset_token: String,
    /// Address of the token that is to be bonded (typically, a TerraSwap LP token)
    pub staking_token: String,
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
    /// @notice Return the amount of LP tokens bonded to the staking contract
    pub fn query_bond(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
    ) -> StdResult<Uint128> {
        let (bond_amount, _) = self._query_reward_info(querier, staker)?;
        Ok(bond_amount)
    }

    /// @notice Return the amount of claimable reward
    pub fn query_reward(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
    ) -> StdResult<Uint128> {
        let (_, pending_reward) = self._query_reward_info(querier, staker)?;
        Ok(pending_reward)
    }

    /// @notice Generate a message for bonding LP tokens
    pub fn bond_msg(&self, amount: Uint128) -> StdResult<SubMsg> {
        match self {
            Self::Anchor(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.contract_addr.clone(),
                    amount,
                    msg: to_binary(&anchor_staking::Cw20HookMsg::Bond {})?,
                })?,
                funds: vec![],
            })),
            Self::Mirror(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: config.contract_addr.clone(),
                    amount,
                    msg: to_binary(&mirror_staking::Cw20HookMsg::Bond {
                        asset_token: config.asset_token.clone(),
                    })?,
                })?,
                funds: vec![],
            })),
        }
    }

    /// @notice Generate a message for unbonding LP tokens
    pub fn unbond_msg(&self, amount: Uint128) -> StdResult<SubMsg> {
        match self {
            Self::Anchor(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                msg: to_binary(&anchor_staking::ExecuteMsg::Unbond {
                    amount,
                })?,
                funds: vec![],
            })),
            Self::Mirror(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                msg: to_binary(&mirror_staking::ExecuteMsg::Unbond {
                    asset_token: config.asset_token.clone(),
                    amount,
                })?,
                funds: vec![],
            })),
        }
    }

    /// @notice Generate a message for claiming staking rewards
    pub fn withdraw_msg(&self) -> StdResult<SubMsg> {
        match self {
            Self::Anchor(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                msg: to_binary(&anchor_staking::ExecuteMsg::Withdraw {})?,
                funds: vec![],
            })),
            Self::Mirror(config) => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: config.contract_addr.clone(),
                msg: to_binary(&mirror_staking::ExecuteMsg::Withdraw {
                    asset_token: Some(config.asset_token.clone()),
                })?,
                funds: vec![],
            })),
        }
    }

    /// @notice Return the amounts of 1) bonded `staking_tokens` and 2) claimable reward
    fn _query_reward_info(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
    ) -> StdResult<(Uint128, Uint128)> {
        match self {
            Self::Anchor(config) => {
                let response: anchor_staking::StakerInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.clone(),
                        msg: to_binary(&anchor_staking::QueryMsg::StakerInfo {
                            staker: String::from(staker),
                            block_height: None,
                        })?,
                    }))?;
                Ok((response.bond_amount, response.pending_reward))
            }
            Self::Mirror(config) => {
                let response: mirror_staking::RewardInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.clone(),
                        msg: to_binary(&mirror_staking::QueryMsg::RewardInfo {
                            staker_addr: String::from(staker),
                            asset_token: Some(config.asset_token.clone()),
                        })?,
                    }))?;
                if response.reward_infos.len() > 0 {
                    let reward_info = &response.reward_infos[0];
                    Ok((reward_info.bond_amount, reward_info.pending_reward))
                } else {
                    Ok((Uint128::zero(), Uint128::zero()))
                }
            }
        }
    }
}
