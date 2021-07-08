use cosmwasm_std::{Decimal, Uint128};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    pub exchange_rate: Decimal,
    pub er_threshold: Decimal,
    pub peg_recovery_fee: Decimal,
    pub requested_with_fee: Uint128,
}
