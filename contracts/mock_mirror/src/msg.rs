use cosmwasm_std::HumanAddr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// We overwrite mirror_protocol::staking::InitMsg with this simplified message type
// All other message types are kept the same with the official crate
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Address of MIR token
    pub mirror_token: HumanAddr,
    /// Address of the token to be staked (MIR or mAsset)
    pub asset_token: HumanAddr,
    /// Address of MIR-UST LP token
    pub staking_token: HumanAddr,
}
