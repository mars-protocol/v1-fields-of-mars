use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, Coin, CosmosMsg, Decimal, Extern, HumanAddr, Querier,
    QueryRequest, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetInfo};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    ProvideLiquidity {
        assets: [Asset; 2],
        slippage_tolerance: Option<Decimal>,
    },
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<HumanAddr>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    Swap {
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<HumanAddr>,
    },
    WithdrawLiquidity {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Pool {},
    Simulation {
        offer_asset: Asset,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolResponse {
    pub assets: [Asset; 2],
    pub total_share: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SimulationResponse {
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub commission_amount: Uint128,
}

//----------------------------------------------------------------------------------------
// Adapter
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Swap {
    /// Address of the TerraSwap pair contract
    pub pair: HumanAddr,
    /// Address of the TerraSwap LP token
    pub share_token: HumanAddr,
}

impl Swap {
    /// @notice Convert `Swap` to `SwapRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<SwapRaw> {
        Ok(SwapRaw {
            pair: deps.api.canonical_address(&self.pair)?,
            share_token: deps.api.canonical_address(&self.share_token)?,
        })
    }

    /// @notice Generate messages for providing specified assets
    pub fn provide_messages(&self, assets: [Asset; 2]) -> StdResult<Vec<CosmosMsg>> {
        let mut messages: Vec<CosmosMsg> = vec![];
        let mut send: Vec<Coin> = vec![];

        for asset in assets.iter() {
            match &asset.info {
                AssetInfo::Token {
                    contract_addr,
                } => messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: HumanAddr::from(contract_addr),
                    send: vec![],
                    msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                        spender: self.pair.clone(),
                        amount: asset.amount,
                        expires: None,
                    })?,
                })),
                AssetInfo::NativeToken {
                    denom,
                } => send.push(Coin {
                    denom: String::from(denom),
                    amount: asset.amount,
                }),
            }
        }

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.pair.clone(),
            send,
            msg: to_binary(&HandleMsg::ProvideLiquidity {
                assets,
                slippage_tolerance: None,
            })?,
        }));

        Ok(messages)
    }

    /// @notice Generate msg for removing liquidity by burning specified amount of LP tokens
    /// @param amount Amount of LP tokens to burn
    pub fn withdraw_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.share_token.clone(),
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Send {
                contract: self.pair.clone(),
                amount,
                msg: Some(to_binary(&Cw20HookMsg::WithdrawLiquidity {})?),
            })?,
        }))
    }

    /// @notice Generate msg for swapping specified asset
    pub fn swap_message(&self, asset: Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Token {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: self.pair.clone(),
                    amount: asset.amount,
                    msg: Some(to_binary(&Cw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?),
                })?,
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.pair.clone(),
                send: vec![Coin {
                    denom: String::from(denom),
                    amount: asset.amount,
                }],
                msg: to_binary(&HandleMsg::Swap {
                    offer_asset: asset,
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })?,
            })),
        }
    }

    /// @notice Query and parse pool info, including depths of the two assets as well as
    //  the supply of LP tokens.
    pub fn query_pool<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        long_info: &AssetInfo,
        short_info: &AssetInfo,
    ) -> StdResult<PoolResponseParsed> {
        let response: PoolResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.pair.clone(),
                msg: to_binary(&QueryMsg::Pool {})?,
            }))?;
        Ok(PoolResponseParsed::parse(&response, &long_info, &short_info))
    }

    /// @notice Query the return amount of a swap
    pub fn query_simulation<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        offer_asset: Asset,
    ) -> StdResult<Uint128> {
        let response: SimulationResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.pair.clone(),
                msg: to_binary(&QueryMsg::Simulation {
                    offer_asset,
                })?,
            }))?;
        Ok(response.return_amount)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PoolResponseParsed {
    /// Amount of long asset held by the pool
    pub long_depth: Uint128,
    /// Amount of short asset held by the pool
    pub short_depth: Uint128,
    /// Total supply of the LP token
    pub share_supply: Uint128,
}

impl PoolResponseParsed {
    pub fn parse(
        response: &PoolResponse,
        long_info: &AssetInfo,
        short_info: &AssetInfo,
    ) -> Self {
        let long_depth = response
            .assets
            .iter()
            .find(|asset| asset.info.equals(&long_info))
            .unwrap()
            .amount;
        let short_depth = response
            .assets
            .iter()
            .find(|asset| asset.info.equals(&short_info))
            .unwrap()
            .amount;
        PoolResponseParsed {
            long_depth,
            short_depth,
            share_supply: response.total_share,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SwapRaw {
    /// Address of the TerraSwap pair contract
    pub pair: CanonicalAddr,
    /// Address of the TerraSwap LP token
    pub share_token: CanonicalAddr,
}

impl SwapRaw {
    /// @notice Convert `SwapRaw` to `Swap`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<Swap> {
        Ok(Swap {
            pair: deps.api.human_address(&self.pair)?,
            share_token: deps.api.human_address(&self.share_token)?,
        })
    }
}

//----------------------------------------------------------------------------------------
// Unit Tests
//----------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pool_response() {
        let response = PoolResponse {
            assets: [
                Asset {
                    info: AssetInfo::Token {
                        contract_addr: HumanAddr("addr1234".to_string()),
                    },
                    amount: Uint128::from(123456u128),
                },
                Asset {
                    info: AssetInfo::NativeToken {
                        denom: "uusd".to_string(),
                    },
                    amount: Uint128::from(69420u128),
                },
            ],
            total_share: Uint128::from(88888u128),
        };

        let parsed = PoolResponseParsed::parse(
            &response,
            &AssetInfo::Token {
                contract_addr: HumanAddr("addr1234".to_string()),
            },
            &AssetInfo::NativeToken {
                denom: "uusd".to_string(),
            },
        );

        assert_eq!(
            parsed,
            PoolResponseParsed {
                long_depth: Uint128::from(123456u128),
                short_depth: Uint128::from(69420u128),
                share_supply: Uint128::from(88888u128)
            }
        );
    }
}
