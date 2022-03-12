use std::str::FromStr;

use cosmwasm_std::{
    to_binary, Addr, Api, Coin, CosmosMsg, Decimal, Event, QuerierWrapper, QueryRequest, StdError,
    StdResult, SubMsg, Uint128, WasmMsg, WasmQuery,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use astroport::pair::{Cw20HookMsg, ExecuteMsg, PoolResponse, QueryMsg, ReverseSimulationResponse};

use cw_asset::{Asset, AssetInfo, AssetUnchecked};

//--------------------------------------------------------------------------------------------------
// Pair
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairBase<T> {
    /// Address of the Astroport contract_addr contract
    pub contract_addr: T,
    /// Address of the Astroport LP token
    pub liquidity_token: T,
}

pub type PairUnchecked = PairBase<String>;
pub type Pair = PairBase<Addr>;

impl From<Pair> for PairUnchecked {
    fn from(pair: Pair) -> Self {
        PairUnchecked {
            contract_addr: pair.contract_addr.to_string(),
            liquidity_token: pair.liquidity_token.to_string(),
        }
    }
}

impl PairUnchecked {
    pub fn check(&self, api: &dyn Api) -> StdResult<Pair> {
        Ok(Pair {
            contract_addr: api.addr_validate(&self.contract_addr)?,
            liquidity_token: api.addr_validate(&self.liquidity_token)?,
        })
    }
}

impl Pair {
    /// Create a new pair instance
    pub fn new(contract_addr: &Addr, liquidity_token: &Addr) -> Self {
        Self {
            contract_addr: contract_addr.clone(),
            liquidity_token: liquidity_token.clone(),
        }
    }

    /// Generate submessages for providing specified assets
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn provide_submsgs(
        &self,
        id: u64,
        assets: &[Asset; 2],
        slippage_tolerance: Option<Decimal>,
    ) -> StdResult<Vec<SubMsg>> {
        let mut submsgs: Vec<SubMsg> = vec![];
        let mut funds: Vec<Coin> = vec![];

        for asset in assets.iter() {
            match &asset.info {
                AssetInfo::Cw20(contract_addr) => submsgs.push(SubMsg::new(WasmMsg::Execute {
                    contract_addr: contract_addr.to_string(),
                    msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
                        spender: self.contract_addr.to_string(),
                        amount: asset.amount,
                        expires: None,
                    })?,
                    funds: vec![],
                })),
                AssetInfo::Native(denom) => funds.push(Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }),
            }
        }

        submsgs.push(SubMsg::reply_on_success(
            WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&ExecuteMsg::ProvideLiquidity {
                    assets: [assets[0].clone().into(), assets[1].clone().into()],
                    slippage_tolerance,
                    auto_stake: None,
                    receiver: None,
                })?,
                funds,
            },
            id,
        ));

        Ok(submsgs)
    }

    /// Generate submsg for removing liquidity by burning specified amount of shares
    pub fn withdraw_submsg(&self, id: u64, shares: Uint128) -> StdResult<SubMsg> {
        Ok(SubMsg::reply_on_success(
            WasmMsg::Execute {
                contract_addr: self.liquidity_token.to_string(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: self.contract_addr.to_string(),
                    amount: shares,
                    msg: to_binary(&Cw20HookMsg::WithdrawLiquidity {})?,
                })?,
                funds: vec![],
            },
            id,
        ))
    }

    /// @notice Generate submsg for swapping specified asset
    /// NOTE: For now, we don't specify a slippage tolerance
    pub fn swap_submsg(
        &self,
        id: u64,
        asset: &Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
    ) -> StdResult<SubMsg> {
        let msg = match &asset.info {
            AssetInfo::Cw20(_) => asset.send_msg(
                &self.contract_addr,
                to_binary(&Cw20HookMsg::Swap {
                    belief_price,
                    max_spread,
                    to: None,
                })?,
            )?,
            AssetInfo::Native(denom) => CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&ExecuteMsg::Swap {
                    offer_asset: asset.clone().into(),
                    belief_price,
                    max_spread,
                    to: None,
                })?,
                funds: vec![Coin {
                    denom: denom.clone(),
                    amount: asset.amount,
                }],
            }),
        };
        Ok(SubMsg::reply_on_success(msg, id))
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
            .find(|asset| asset.info == *primary_asset_info)
            .ok_or_else(|| StdError::generic_err("cannot find primary asset in pool response"))?
            .amount;

        let secondary_asset_depth = response
            .assets
            .iter()
            .find(|asset| asset.info == *secondary_asset_info)
            .ok_or_else(|| StdError::generic_err("cannot find secondary asset in pool response"))?
            .amount;

        Ok((primary_asset_depth, secondary_asset_depth, response.total_share))
    }

    /// Calculate how much offer asset is needed to return a specified amount of ask asset
    pub fn query_reverse_simulate(
        &self,
        querier: &QuerierWrapper,
        ask_asset: &Asset,
    ) -> StdResult<Uint128> {
        let response: ReverseSimulationResponse =
            querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.to_string(),
                msg: to_binary(&QueryMsg::ReverseSimulation {
                    ask_asset: ask_asset.into(),
                })?,
            }))?;
        Ok(response.offer_amount)
    }

    /// Find the return amount when swapping in an Astroport pool
    pub fn parse_swap_events(events: &[Event]) -> StdResult<AssetUnchecked> {
        let event = events
            .iter()
            .find(|event| event_contains_attr(event, "action", "swap"))
            .ok_or_else(|| StdError::generic_err("cannot find `swap` event"))?;

        let ask_asset_str = event
            .attributes
            .iter()
            .cloned()
            .find(|attr| attr.key == "ask_asset")
            .ok_or_else(|| StdError::generic_err("cannot find `ask_asset` attribute"))?
            .value;

        let return_amount_str = event
            .attributes
            .iter()
            .cloned()
            .find(|attr| attr.key == "return_amount")
            .ok_or_else(|| StdError::generic_err("cannot find `return_amount` attribute"))?
            .value;

        let tax_amount_str = event
            .attributes
            .iter()
            .cloned()
            .find(|attr| attr.key == "tax_amount")
            .ok_or_else(|| StdError::generic_err("cannot find `tax_amount` attribute"))?
            .value;

        let return_amount = Uint128::from_str(&return_amount_str)?;
        let tax_amount = Uint128::from_str(&tax_amount_str)?;
        let return_amount_after_tax = return_amount.checked_sub(tax_amount)?;

        // If the asset's label starts with `terra` then we assume it is a CW20
        // Otherwise, assume it is a native
        // Not a very clean way of doin this, but ok
        if ask_asset_str.starts_with("terra") {
            Ok(AssetUnchecked::cw20(ask_asset_str, return_amount_after_tax))
        } else {
            Ok(AssetUnchecked::native(ask_asset_str, return_amount_after_tax))
        }
    }

    /// Find the amount of share tokens minted when providing liquidity to an Astroport pool
    pub fn parse_provide_events(events: &[Event]) -> StdResult<Uint128> {
        let event = events
            .iter()
            .find(|event| event_contains_attr(event, "action", "provide_liquidity"))
            .ok_or_else(|| StdError::generic_err("cannot find `provide_liquidity` event"))?;

        let share_str = event
            .attributes
            .iter()
            .cloned()
            .find(|attr| attr.key == "share")
            .ok_or_else(|| StdError::generic_err("cannot find `share` attribute"))?
            .value;

        Uint128::from_str(&share_str)
    }

    /// Find the amount of assets refunded when withdrawing liquidity from an Astroport pool
    /// Returns a 2-tuple: (primary_asset_withdrawn, secondary_asset_withdrawn)
    pub fn parse_withdraw_events(
        events: &[Event],
        primary_asset_info: &AssetInfo,
        secondary_asset_info: &AssetInfo,
    ) -> StdResult<(Asset, Asset)> {
        let event = events
            .iter()
            .find(|event| event_contains_attr(event, "action", "withdraw_liquidity"))
            .ok_or_else(|| StdError::generic_err("cannot find `withdraw_liquidity` event"))?;

        let asset_strs: Vec<&str> = event
            .attributes
            .iter()
            .find(|attr| attr.key == "refund_assets")
            .ok_or_else(|| StdError::generic_err("cannot find `refund_assets` attribute"))?
            .value
            .split(", ")
            .collect();

        let primary_asset_label = get_asset_label(primary_asset_info);
        let primary_withdrawn_amount_str = asset_strs
            .iter()
            .find(|asset_str| asset_str.contains(&primary_asset_label))
            .map(|asset_str| asset_str.replace(&primary_asset_label, ""))
            .ok_or_else(|| StdError::generic_err("failed to parse primary withdrawn amount"))?;

        let secondary_asset_label = get_asset_label(secondary_asset_info);
        let secondary_withdrawn_amount_str = asset_strs
            .iter()
            .find(|asset_str| asset_str.contains(&secondary_asset_label))
            .map(|asset_str| asset_str.replace(&secondary_asset_label, ""))
            .ok_or_else(|| StdError::generic_err("failed to parse secondary withdrawn amount"))?;

        let primary_asset_withdrawn = Asset::new(
            primary_asset_info.clone(),
            Uint128::from_str(&primary_withdrawn_amount_str)?,
        );
        let secondary_asset_withdrawn = Asset::new(
            secondary_asset_info.clone(),
            Uint128::from_str(&secondary_withdrawn_amount_str)?,
        );

        Ok((primary_asset_withdrawn, secondary_asset_withdrawn))
    }
}

fn event_contains_attr(event: &Event, key: &str, value: &str) -> bool {
    event.attributes.iter().any(|attr| attr.key == key && attr.value == value)
}

fn get_asset_label(asset_info: &AssetInfo) -> String {
    match asset_info {
        AssetInfo::Cw20(contract_addr) => contract_addr.into(),
        AssetInfo::Native(denom) => denom.clone(),
    }
}