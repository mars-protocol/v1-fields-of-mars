use cosmwasm_std::{
    to_binary, Addr, Api, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmMsg,
    WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use anchor_token::staking as anchor_staking;
use mirror_protocol::staking as mirror_staking;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakingConfigBase<T> {
    /// Address of the staking contract
    pub contract_addr: T,
    /// Address of ANC, MIR, or mAsset token; refer to Mirror contract for definition
    pub asset_token: T,
    /// Address of ANC-UST, MIR-UST, or mAsset-UST LP token
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

impl StakingConfigUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<StakingConfig> {
        Ok(StakingConfig {
            contract_addr: api.addr_validate(&self.contract_addr)?,
            asset_token: api.addr_validate(&self.asset_token)?,
            staking_token: api.addr_validate(&self.staking_token)?,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StakingBase<T> {
    /// Anchor staking contract, or those forked from it, e.g. Pylon
    Anchor(StakingConfigBase<T>),
    /// Mirror V2 staking contract
    Mirror(StakingConfigBase<T>),
}

pub type StakingUnchecked = StakingBase<String>;
pub type Staking = StakingBase<Addr>;

impl From<Staking> for StakingUnchecked {
    fn from(staking: Staking) -> Self {
        match staking {
            Staking::Anchor(config) => StakingUnchecked::Anchor(config.into()),
            Staking::Mirror(config) => StakingUnchecked::Mirror(config.into()),
        }
    }
}

impl StakingUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Staking> {
        Ok(match self {
            StakingUnchecked::Anchor(config) => Staking::Anchor(config.check(api)?),
            StakingUnchecked::Mirror(config) => Staking::Mirror(config.check(api)?),
        })
    }
}

impl Staking {
    pub fn get_config(&self) -> StakingConfig {
        let config = match self {
            Staking::Anchor(config) => config,
            Staking::Mirror(config) => config,
        };
        config.clone()
    }

    /// Generate a message for bonding LP tokens
    pub fn bond_msg(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        let config = self.get_config();

        let msg = match self {
            Staking::Anchor(..) => to_binary(&anchor_staking::Cw20HookMsg::Bond {})?,
            Staking::Mirror(config) => to_binary(&mirror_staking::Cw20HookMsg::Bond {
                asset_token: config.asset_token.to_string(),
            })?,
        };

        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.staking_token.to_string(),
            msg: to_binary(&&Cw20ExecuteMsg::Send {
                contract: config.contract_addr.to_string(),
                amount,
                msg,
            })?,
            funds: vec![],
        }))
    }

    /// Generate a message for unbonding LP tokens
    pub fn unbond_msg(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        let config = self.get_config();

        let msg = match self {
            Staking::Anchor(..) => to_binary(&anchor_staking::ExecuteMsg::Unbond {
                amount,
            })?,
            Staking::Mirror(config) => to_binary(&mirror_staking::ExecuteMsg::Unbond {
                asset_token: config.asset_token.to_string(),
                amount,
            })?,
        };

        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.contract_addr.to_string(),
            msg,
            funds: vec![],
        }))
    }

    /// Generate a message for claiming staking rewards
    pub fn withdraw_msg(&self) -> StdResult<CosmosMsg> {
        let config = self.get_config();

        let msg = match self {
            Staking::Anchor(..) => to_binary(&anchor_staking::ExecuteMsg::Withdraw {})?,
            Staking::Mirror(config) => to_binary(&mirror_staking::ExecuteMsg::Withdraw {
                asset_token: Some(config.asset_token.to_string()),
            })?,
        };

        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.contract_addr.to_string(),
            msg,
            funds: vec![],
        }))
    }

    /// Return the amounts of 1) bonded `staking_tokens` and 2) claimable reward
    pub fn query_reward_info(
        &self,
        querier: &QuerierWrapper,
        staker_addr: &Addr,
        block_height: u64,
    ) -> StdResult<(Uint128, Uint128)> {
        let (bonded_amount, pending_reward_amount) = match self {
            Staking::Anchor(config) => {
                let response: anchor_staking::StakerInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&anchor_staking::QueryMsg::StakerInfo {
                            staker: staker_addr.to_string(),
                            block_height: Some(block_height), // NOTE: for anchor, block height must be provided
                        })?,
                    }))?;

                (response.bond_amount, response.pending_reward)
            }

            Staking::Mirror(config) => {
                let response: mirror_staking::RewardInfoResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.contract_addr.to_string(),
                        msg: to_binary(&mirror_staking::QueryMsg::RewardInfo {
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
