use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, Decimal, QuerierWrapper, QueryRequest, StdError,
    StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{AssetChecked, AssetInfoChecked};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Pool<T> {
    /// Address of the Astroport pair contract
    pub pair: T,
    /// Address of the Astroport LP token
    pub share_token: T,
}

pub type PoolUnchecked = Pool<String>;
pub type PoolChecked = Pool<Addr>;

impl From<PoolChecked> for PoolUnchecked {
    fn from(checked: PoolChecked) -> Self {
        PoolUnchecked {
            pair: checked.pair.to_string(),
            share_token: checked.share_token.to_string(),
        }
    }
}

impl PoolUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<PoolChecked> {
        let checked = PoolChecked {
            pair: api.addr_validate(&self.pair)?,
            share_token: api.addr_validate(&self.share_token)?,
        };

        Ok(checked)
    }
}

impl PoolChecked {
    /// Generate messages for providing specified assets
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn provide_msgs(&self, assets: &[AssetChecked; 2]) -> StdResult<Vec<CosmosMsg>> {
        let mut messages: Vec<CosmosMsg> = vec![];
        let mut funds: Vec<Coin> = vec![];

        for asset in assets.iter() {
            match &asset.info {
                AssetInfoChecked::Token { contract_addr } => {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: contract_addr.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                            spender: self.pair.to_string(),
                            amount: asset.amount,
                            expires: None,
                        })?,
                        funds: vec![],
                    }))
                }
                AssetInfoChecked::NativeToken { denom } => funds.push(Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }),
            }
        }

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.pair.to_string(),
            msg: to_binary(&msg::HandleMsg::ProvideLiquidity {
                assets: [assets[0].clone(), assets[1].clone()],
                slippage_tolerance: None, // to be added in a future version
            })?,
            funds,
        }));

        Ok(messages)
    }

    /// Generate msg for removing liquidity by burning specified amount of shares
    pub fn withdraw_msg(&self, shares: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.share_token.to_string(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: self.pair.to_string(),
                amount: shares,
                msg: to_binary(&msg::Cw20HookMsg::WithdrawLiquidity {})?,
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate msg for swapping specified asset
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn swap_msg(&self, asset: &AssetChecked) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfoChecked::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.pair.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&msg::Cw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?,
                })?,
                funds: vec![],
            })),

            AssetInfoChecked::NativeToken { denom } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.pair.to_string(),
                msg: to_binary(&msg::HandleMsg::Swap {
                    offer_asset: asset.clone(),
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })?,
                funds: vec![Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }],
            })),
        }
    }

    pub fn query_pool(
        &self,
        querier: &QuerierWrapper,
        primary_asset_info: &AssetInfoChecked,
        secondary_asset_info: &AssetInfoChecked,
    ) -> StdResult<msg::PoolResponseParsed> {
        let response: msg::PoolResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.pair.to_string(),
            msg: to_binary(&msg::QueryMsg::Pool {})?,
        }))?;

        let primary_asset_depth = response
            .assets
            .iter()
            .find(|asset| &asset.info == primary_asset_info)
            .ok_or_else(|| StdError::generic_err("Cannot find primary asset in pool response"))?
            .amount;

        let secondary_asset_depth = response
            .assets
            .iter()
            .find(|asset| &asset.info == secondary_asset_info)
            .ok_or_else(|| StdError::generic_err("Cannot find secondary asset in pool response"))?
            .amount;

        Ok(msg::PoolResponseParsed {
            primary_asset_depth,
            secondary_asset_depth,
            share_token_supply: response.total_share,
        })
    }

    /// @notice Query an account's balance of the pool's share token
    pub fn query_share(&self, querier: &QuerierWrapper, account: &Addr) -> StdResult<Uint128> {
        let share_token = AssetInfoChecked::Token {
            contract_addr: self.share_token.clone(),
        };

        share_token.query_balance(querier, account)
    }
}

pub mod msg {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum HandleMsg {
        Receive(Cw20ReceiveMsg),
        ProvideLiquidity {
            assets: [AssetChecked; 2],
            slippage_tolerance: Option<Decimal>,
        },
        Swap {
            offer_asset: AssetChecked,
            belief_price: Option<Decimal>,
            max_spread: Option<Decimal>,
            to: Option<String>,
        },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum Cw20HookMsg {
        Swap {
            belief_price: Option<Decimal>,
            max_spread: Option<Decimal>,
            to: Option<String>,
        },
        WithdrawLiquidity {},
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    #[serde(rename_all = "snake_case")]
    pub enum QueryMsg {
        Pool {},
        Simulation { offer_asset: AssetChecked },
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct PoolResponse {
        pub assets: [AssetChecked; 2],
        pub total_share: Uint128,
    }

    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct SimulationResponse {
        pub return_amount: Uint128,
        pub spread_amount: Uint128,
        pub commission_amount: Uint128,
    }

    /// This message type is not part of Astroport
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
    pub struct PoolResponseParsed {
        /// Amount of primary asset in the pool
        pub primary_asset_depth: Uint128,
        /// Amount of secondary asset in the pool
        pub secondary_asset_depth: Uint128,
        /// Total supply of the LP token
        pub share_token_supply: Uint128,
    }
}
