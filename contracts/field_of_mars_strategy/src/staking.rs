use anchor_token::staking as anchor;
use cosmwasm_std::{
    to_binary, Api, CosmosMsg, Extern, HumanAddr, Querier, QueryRequest, StdError,
    StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::Cw20HandleMsg;
use mirror_protocol::staking as mirror;

use crate::state::Config;

pub struct StakingConfig {
    /// Address of the staking contract
    pub address: HumanAddr,
    /// Address of the asset token (MIR, mAsset, ANC)
    pub asset_token: HumanAddr,
    /// Address of the token that is to be bonded (typically, a Terraswap LP token)
    pub staking_token: HumanAddr,
}

pub enum StakingContract {
    /// Anchor staking contract
    Anchor(StakingConfig),
    /// Mirror V2 staking contract
    Mirror(StakingConfig),
}

impl StakingContract {
    /// @notice Generate a staking contract object from config info
    pub fn from_config<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        config: &Config,
    ) -> StdResult<Self> {
        match config.staking_type.as_str() {
            "anchor" => Ok(StakingContract::Anchor(StakingConfig {
                address: deps.api.human_address(&config.staking_contract)?,
                asset_token: deps.api.human_address(&config.asset_token)?,
                staking_token: deps.api.human_address(&config.pool_token)?,
            })),
            "mirror" => Ok(StakingContract::Mirror(StakingConfig {
                address: deps.api.human_address(&config.staking_contract)?,
                asset_token: deps.api.human_address(&config.asset_token)?,
                staking_token: deps.api.human_address(&config.pool_token)?,
            })),
            _ => Err(StdError::generic_err("Invalid staking contract type")),
        }
    }

    /// @notice Return the amount of LP tokens bonded to the staking contract
    pub fn query_bond_amount<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<Uint128> {
        let (bond_amount, _) = self._query_reward_info(deps, staker)?;
        Ok(bond_amount)
    }

    /// @notice Return the amount of claimable reward
    pub fn query_reward_amount<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<Uint128> {
        let (_, pending_reward) = self._query_reward_info(deps, staker)?;
        Ok(pending_reward)
    }

    /// @notice Generate a message for bonding LP tokens
    pub fn bond_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match self {
            StakingContract::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: config.address.clone(),
                    amount,
                    msg: Some(to_binary(&anchor::Cw20HookMsg::Bond {})?),
                })?,
            })),
            StakingContract::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.staking_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: config.address.clone(),
                    amount,
                    msg: Some(to_binary(&mirror::Cw20HookMsg::Bond {
                        asset_token: config.asset_token.clone(),
                    })?),
                })?,
            })),
        }
    }

    /// @notice Generate a message for unbonding LP tokens
    pub fn unbond_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match self {
            StakingContract::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.address.clone(),
                send: vec![],
                msg: to_binary(&anchor::HandleMsg::Unbond {
                    amount,
                })?,
            })),
            StakingContract::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.address.clone(),
                send: vec![],
                msg: to_binary(&mirror::HandleMsg::Unbond {
                    asset_token: config.asset_token.clone(),
                    amount,
                })?,
            })),
        }
    }

    /// @notice Generate a message for claiming staking rewards
    pub fn withdraw_message(&self) -> StdResult<CosmosMsg> {
        match self {
            StakingContract::Anchor(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.address.clone(),
                send: vec![],
                msg: to_binary(&anchor::HandleMsg::Withdraw {})?,
            })),
            StakingContract::Mirror(config) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.address.clone(),
                send: vec![],
                msg: to_binary(&mirror::HandleMsg::Withdraw {
                    asset_token: Some(config.asset_token.clone()),
                })?,
            })),
        }
    }

    /// @notice Return tuple of two numbers: (bond_amount, pending_rewards)
    fn _query_reward_info<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        staker: &HumanAddr,
    ) -> StdResult<(Uint128, Uint128)> {
        match self {
            StakingContract::Anchor(config) => {
                let response = deps.querier.query::<anchor::StakerInfoResponse>(
                    &QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.address.clone(),
                        msg: to_binary(&anchor::QueryMsg::StakerInfo {
                            staker: HumanAddr::from(staker),
                            block_height: None,
                        })?,
                    }),
                )?;
                Ok((response.bond_amount, response.pending_reward))
            }
            StakingContract::Mirror(config) => {
                let response = deps.querier.query::<mirror::RewardInfoResponse>(
                    &QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: config.address.clone(),
                        msg: to_binary(&mirror::QueryMsg::RewardInfo {
                            staker_addr: HumanAddr::from(staker),
                            asset_token: Some(config.asset_token.clone()),
                        })?,
                    }),
                )?;
                Ok(if response.reward_infos.len() > 0 {
                    (
                        response.reward_infos[0].bond_amount,
                        response.reward_infos[0].pending_reward,
                    )
                } else {
                    (Uint128::zero(), Uint128::zero())
                })
            }
        }
    }
}
