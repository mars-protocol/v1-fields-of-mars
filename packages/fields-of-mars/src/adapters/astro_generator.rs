use cosmwasm_std::{
    to_binary, Addr, Api, CosmosMsg, QuerierWrapper, QueryRequest, StdResult, Uint128, WasmMsg,
    WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::generator::{
    Cw20HookMsg, ExecuteMsg, PendingTokenResponse, QueryMsg, RewardInfoResponse,
};

use cw_asset::{Asset, AssetList};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct GeneratorBase<T> {
    /// Address of the Astro generator contract
    pub contract_addr: T,
}

pub type GeneratorUnchecked = GeneratorBase<String>;
pub type Generator = GeneratorBase<Addr>;

impl From<Generator> for GeneratorUnchecked {
    fn from(generator: Generator) -> Self {
        Self {
            contract_addr: generator.contract_addr.to_string(),
        }
    }
}

impl GeneratorUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Generator> {
        Ok(Generator {
            contract_addr: api.addr_validate(&self.contract_addr)?,
        })
    }
}

impl Generator {
    /// Create a new `Generator` instance
    pub fn new(contract_addr: &Addr) -> Self {
        Self {
            contract_addr: contract_addr.clone(),
        }
    }

    /// Create a message for depositing a liquidity token of the specified amount on behalf of the
    /// sender
    ///
    /// NOTE: pending rewards are automatically withdrawn during the execution of this message
    pub fn bond_msg(&self, liquidity_token: &Addr, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: liquidity_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: self.contract_addr.to_string(),
                amount,
                msg: to_binary(&Cw20HookMsg::Deposit {})?,
            })?,
            funds: vec![],
        }))
    }

    /// Create a message for withdrawing a liquidity token of the specified amount
    ///
    /// NOTE: pending rewards are automatically withdrawn during the execution of this message
    pub fn unbond_msg(&self, liquidity_token: &Addr, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Withdraw {
                lp_token: liquidity_token.clone(), // this ExecuteMsg takes Addr instead of String
                amount,
            })?,
            funds: vec![],
        }))
    }

    /// Create a message for claiming pending rewards
    ///
    /// NOTE: this is simply a message withdrawing zero liquidity tokens
    pub fn claim_rewards_msg(&self, liquidity_token: &Addr) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Withdraw {
                lp_token: liquidity_token.clone(),
                amount: Uint128::zero(), // claim rewards by withdrawing zero liquidity tokens
            })?,
            funds: vec![],
        }))
    }

    /// Query the amount of a liquidity token currently staked by the staker
    pub fn query_bonded_amount(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
        liquidity_token: &Addr,
    ) -> StdResult<Uint128> {
        querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&QueryMsg::Deposit {
                user: staker.clone(),
                lp_token: liquidity_token.clone(),
            })?,
        }))
    }

    /// Query the amounts of pending rewards claimable by the staker. Returns a vector of assets
    pub fn query_rewards(
        &self,
        querier: &QuerierWrapper,
        staker: &Addr,
        liquidity_token: &Addr,
    ) -> StdResult<AssetList> {
        let reward_info: RewardInfoResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&QueryMsg::RewardInfo {
                    lp_token: liquidity_token.clone(),
                })?,
            }))?;

        let pending_tokens: PendingTokenResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&QueryMsg::PendingToken {
                    user: staker.clone(),
                    lp_token: liquidity_token.clone(),
                })?,
            }))?;

        let mut rewards = AssetList::from(vec![Asset::cw20(
            reward_info.base_reward_token,
            pending_tokens.pending,
        )]);

        if let Some(proxy_reward_token) = reward_info.proxy_reward_token {
            rewards.add(&Asset::cw20(
                proxy_reward_token,
                pending_tokens.pending_on_proxy.unwrap_or_else(Uint128::zero),
            ))?;
        }

        rewards.purge(); // remove zero amounts
        Ok(rewards)
    }
}
