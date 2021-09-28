use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, QuerierWrapper, QueryRequest, StdError, StdResult,
    Uint128, WasmMsg, WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use astroport::pair::{Cw20HookMsg, ExecuteMsg, PoolResponse, QueryMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::adapters::{Asset, AssetInfo};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairBase<T> {
    /// Address of the Astroport contract_addr contract
    pub contract_addr: T,
    /// Address of the Astroport LP token
    pub share_token: T,
}

pub type PairUnchecked = PairBase<String>;
pub type Pair = PairBase<Addr>;

impl From<Pair> for PairUnchecked {
    fn from(checked: Pair) -> Self {
        PairUnchecked {
            contract_addr: checked.contract_addr.to_string(),
            share_token: checked.share_token.to_string(),
        }
    }
}

impl PairUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Pair> {
        let checked = Pair {
            contract_addr: api.addr_validate(&self.contract_addr)?,
            share_token: api.addr_validate(&self.share_token)?,
        };

        Ok(checked)
    }
}

impl Pair {
    // INSTANCE CREATION

    pub fn new(contract_addr: &Addr, share_token: &Addr) -> Self {
        Self {
            contract_addr: contract_addr.clone(),
            share_token: share_token.clone(),
        }
    }

    // MESSAGES

    /// Generate messages for providing specified assets
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn provide_msgs(&self, assets: &[Asset; 2]) -> StdResult<Vec<CosmosMsg>> {
        let mut messages: Vec<CosmosMsg> = vec![];
        let mut funds: Vec<Coin> = vec![];

        for asset in assets.iter() {
            match &asset.info {
                AssetInfo::Cw20 { contract_addr } => {
                    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: contract_addr.to_string(),
                        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                            spender: self.contract_addr.to_string(),
                            amount: asset.amount,
                            expires: None,
                        })?,
                        funds: vec![],
                    }))
                }
                AssetInfo::Native { denom } => funds.push(Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }),
            }
        }

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::ProvideLiquidity {
                assets: [assets[0].clone().into(), assets[1].clone().into()],
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
                contract: self.contract_addr.to_string(),
                amount: shares,
                msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {})?,
            })?,
            funds: vec![],
        }))
    }

    /// @notice Generate msg for swapping specified asset
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn swap_msg(&self, asset: &Asset) -> StdResult<CosmosMsg> {
        match &asset.info {
            AssetInfo::Cw20 { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: asset.amount,
                    msg: to_binary(&Cw20HookMsg::Swap {
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?,
                })?,
                funds: vec![],
            })),

            AssetInfo::Native { denom } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&ExecuteMsg::Swap {
                    offer_asset: asset.clone().into(),
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

    /// Query the Astroport pool, parse response, and return the following 3-tuple:
    /// 1. depth of the primary asset
    /// 2. depth of the secondary asset
    /// 3. total supply of the share token
    pub fn query_pool(
        &self,
        querier: &QuerierWrapper,
        primary_asset_info: &AssetInfo,
        secondary_asset_info: &AssetInfo,
    ) -> StdResult<(Uint128, Uint128, Uint128)> {
        let response: PoolResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: self.contract_addr.to_string(),
            msg: to_binary(&QueryMsg::Pool {})?,
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

        Ok((
            primary_asset_depth,
            secondary_asset_depth,
            response.total_share,
        ))
    }

    /// @notice Query an account's balance of the pool's share token
    pub fn query_share(&self, querier: &QuerierWrapper, account: &Addr) -> StdResult<Uint128> {
        let share_token = AssetInfo::Cw20 {
            contract_addr: self.share_token.clone(),
        };

        share_token.query_balance(querier, account)
    }
}
