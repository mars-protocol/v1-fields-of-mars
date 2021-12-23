use cosmwasm_std::Uint128;
use cw20::Cw20ReceiveMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use mars_core::asset::Asset as MarsAsset;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    Borrow {
        asset: MarsAsset,
        amount: Uint128,
    },
    RepayNative {
        denom: String,
    },
    /// NOTE: Only used in mock contract! Not present in actual Red Bank contract
    /// Forcibly set a user's debt amount. Used in tests to simulate the accrual of debts
    SetUserDebt {
        user_address: String,
        denom: String,
        amount: Uint128,
    },
}
