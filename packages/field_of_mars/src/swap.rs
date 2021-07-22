use cosmwasm_std::{
    to_binary, Coin, Decimal, QuerierWrapper, QueryRequest, StdResult, SubMsg, Uint128,
    WasmMsg, WasmQuery,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use integer_sqrt::IntegerSquareRoot;
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
    pub pair: String,
    /// Address of the TerraSwap LP token
    pub share_token: String,
}

impl Swap {
    /// @notice Generate messages for providing specified assets
    pub fn provide_msgs(&self, assets: &[Asset; 2]) -> StdResult<Vec<SubMsg>> {
        let mut messages: Vec<SubMsg> = vec![];
        let mut funds: Vec<Coin> = vec![];

        for asset in assets.iter() {
            match &asset.info {
                AssetInfo::Token {
                    contract_addr,
                } => messages.push(SubMsg::new(WasmMsg::Execute {
                    contract_addr: contract_addr.clone(),
                    msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                        spender: self.pair.clone(),
                        amount: asset.amount,
                        expires: None,
                    })?,
                    funds: vec![],
                })),
                AssetInfo::NativeToken {
                    denom,
                } => funds.push(Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }),
            }
        }

        messages.push(SubMsg::new(WasmMsg::Execute {
            contract_addr: self.pair.clone(),
            msg: to_binary(&HandleMsg::ProvideLiquidity {
                assets: [assets[0].clone(), assets[1].clone()],
                slippage_tolerance: None,
            })?,
            funds,
        }));

        Ok(messages)
    }

    /// @notice Generate msg for removing liquidity by burning specified amount of shares
    /// @param shares Amount of shares to burn
    pub fn withdraw_msg(&self, shares: Uint128) -> StdResult<SubMsg> {
        Ok(SubMsg::new(WasmMsg::Execute {
            contract_addr: self.share_token.clone(),
            msg: to_binary(&Cw20ExecuteMsg::Send {
                contract: self.pair.clone(),
                amount: shares,
                msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {})?,
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate msg for swapping specified asset
    pub fn swap_msg(&self, asset: &Asset) -> StdResult<SubMsg> {
        match &asset.info {
            AssetInfo::Token {
                contract_addr,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: contract_addr.clone(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.pair.clone(),
                    amount: asset.amount,
                    msg: to_binary(&Cw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?,
                })?,
                funds: vec![],
            })),
            AssetInfo::NativeToken {
                denom,
            } => Ok(SubMsg::new(WasmMsg::Execute {
                contract_addr: self.pair.clone(),
                msg: to_binary(&HandleMsg::Swap {
                    offer_asset: asset.clone(),
                    belief_price: None,
                    max_spread: None,
                    to: None,
                })?,
                funds: vec![Coin {
                    denom: String::from(denom),
                    amount: asset.amount,
                }],
            })),
        }
    }

    /// @notice Query and parse pool info, including depths of the two assets as well as
    //  the supply of LP tokens.
    pub fn query_pool(
        &self,
        querier: &QuerierWrapper,
        long_info: &AssetInfo,
        short_info: &AssetInfo,
    ) -> StdResult<PoolResponseParsed> {
        let response: PoolResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.pair.clone(),
                msg: to_binary(&QueryMsg::Pool {})?,
            }))?;
        Ok(PoolResponseParsed::parse(&response, &long_info, &short_info))
    }

    /// @notice Simulate the amount of shares to receive by providing liquidity
    /// @dev Reference:
    /// https://github.com/terraswap/terraswap/blob/master/contracts/terraswap_pair/src/contract.rs#L247
    pub fn simulate_provide(
        &self,
        querier: &QuerierWrapper,
        assets: &[Asset; 2],
    ) -> StdResult<Uint128> {
        let pool_info = self.query_pool(querier, &assets[0].info, &assets[1].info)?;

        let shares = if pool_info.share_supply.is_zero() {
            Uint128::new(
                (assets[0].amount.u128() * assets[1].amount.u128()).integer_sqrt(),
            )
        } else {
            std::cmp::min(
                assets[0]
                    .amount
                    .multiply_ratio(pool_info.share_supply, pool_info.long_depth),
                assets[1]
                    .amount
                    .multiply_ratio(pool_info.share_supply, pool_info.short_depth),
            )
        };

        Ok(shares)
    }

    /// @notice Simulate the amount of assets to receive by removing liquidity
    /// @dev Must deduct tax!!
    pub fn simulate_remove(
        &self,
        querier: &QuerierWrapper,
        shares: Uint128,
        long_info: &AssetInfo,
        short_info: &AssetInfo,
    ) -> StdResult<[Uint128; 2]> {
        let pool_info = self.query_pool(querier, long_info, short_info)?;

        let return_amounts = [
            pool_info.long_depth.multiply_ratio(shares, pool_info.share_supply),
            pool_info.short_depth.multiply_ratio(shares, pool_info.share_supply),
        ];

        let return_amounts_after_tax = [
            long_info.deduct_tax(querier, return_amounts[0])?,
            short_info.deduct_tax(querier, return_amounts[1])?,
        ];

        Ok(return_amounts_after_tax)
    }

    /// @notice Query the return amount of a swap
    pub fn simulate_swap(
        &self,
        querier: &QuerierWrapper,
        offer_asset: &Asset,
    ) -> StdResult<Uint128> {
        let response: SimulationResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.pair.clone(),
                msg: to_binary(&QueryMsg::Simulation {
                    offer_asset: offer_asset.clone(),
                })?,
            }))?;
        Ok(response.return_amount)
    }
}

//----------------------------------------------------------------------------------------
// Helper Type(s)
//----------------------------------------------------------------------------------------

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
        let long_depth =
            response.assets.iter().find(|asset| &asset.info == long_info).unwrap().amount;

        let short_depth = response
            .assets
            .iter()
            .find(|asset| &asset.info == short_info)
            .unwrap()
            .amount;

        Self {
            long_depth,
            short_depth,
            share_supply: response.total_share,
        }
    }
}
