use cosmwasm_std::HumanAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// We overwrite anchor_token::staking::InitMsg with this simplified message type
// All other message types are kept the same with the official crate
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Address of ANC token
    pub anchor_token: HumanAddr,
    /// Address of ANC-UST LP token
    pub staking_token: HumanAddr,
}
