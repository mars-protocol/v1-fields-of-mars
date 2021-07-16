use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    to_binary, Api, CanonicalAddr, Coin, CosmosMsg, Extern, HumanAddr, Querier,
    QueryRequest, StdResult, Storage, Uint128, WasmMsg, WasmQuery,
};
use cw20::{Cw20HandleMsg, Cw20ReceiveMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::asset::{Asset, AssetRaw};

//----------------------------------------------------------------------------------------
// Message Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    Borrow {
        asset: Asset,
        amount: Uint256,
    },
    RepayNative {
        denom: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    RepayCw20 {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Reserve {
        asset: Asset,
    },
    ReservesList {},
    Debt {
        address: HumanAddr,
    },
    UncollateralizedLoanLimit {
        user_address: HumanAddr,
        asset: Asset,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtResponse {
    pub debts: Vec<DebtInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtInfo {
    pub denom: String,
    pub amount: Uint256,
}

//----------------------------------------------------------------------------------------
// Adapter
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBank {
    /// Address of Mars liquidity pool
    pub contract_addr: HumanAddr,
    /// The asset to borrow
    pub borrow_asset: Asset,
}

impl RedBank {
    /// @notice Convert `RedBank` to `RedBankRaw`
    pub fn to_raw<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<RedBankRaw> {
        Ok(RedBankRaw {
            contract_addr: deps.api.canonical_address(&self.contract_addr)?,
            borrow_asset: self.borrow_asset.to_raw(deps)?,
        })
    }

    /// @notice Generate message for borrowing a specified amount of asset
    pub fn borrow_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: self.contract_addr.clone(),
            send: vec![],
            msg: to_binary(&HandleMsg::Borrow {
                asset: self.borrow_asset.clone(),
                amount: Uint256::from(amount),
            })?,
        }))
    }

    /// @notice Generate message for repaying a specified amount of asset
    pub fn repay_message(&self, amount: Uint128) -> StdResult<CosmosMsg> {
        match &self.borrow_asset {
            Asset::Cw20 {
                contract_addr,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from(contract_addr),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: self.contract_addr.clone(),
                    amount,
                    msg: Some(to_binary(&ReceiveMsg::RepayCw20 {})?),
                })?,
            })),
            Asset::Native {
                denom,
            } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: self.contract_addr.clone(),
                send: vec![Coin {
                    denom: String::from(denom),
                    amount,
                }],
                msg: to_binary(&HandleMsg::RepayNative {
                    denom: String::from(denom),
                })?,
            })),
        }
    }

    /// @notice Query the amount of debt a borrower owes to Red Bank
    pub fn query_debt<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
        borrower: &HumanAddr,
    ) -> StdResult<Uint128> {
        let response: DebtResponse =
            deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: self.contract_addr.clone(),
                msg: to_binary(&QueryMsg::Debt {
                    address: HumanAddr::from(borrower),
                })?,
            }))?;

        match response
            .debts
            .iter()
            .find(|debt| debt.denom == self.borrow_asset.query_denom(deps).unwrap())
        {
            Some(debt) => Ok(debt.amount.into()),
            None => Ok(Uint128::zero()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RedBankRaw {
    /// Address of Mars liquidity pool
    pub contract_addr: CanonicalAddr,
    /// The asset to borrow
    pub borrow_asset: AssetRaw,
}

impl RedBankRaw {
    /// @notice Convert `RedBankRaw` to `RedBank`
    pub fn to_normal<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<RedBank> {
        Ok(RedBank {
            contract_addr: deps.api.human_address(&self.contract_addr)?,
            borrow_asset: self.borrow_asset.to_normal(deps)?,
        })
    }
}
