use cosmwasm_bignumber::Decimal256;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    // User's debt = deposit_amount * mock_interest_rate
    pub mock_interest_rate: Option<Decimal256>,
}

pub type MigrateMsg = InitMsg;
