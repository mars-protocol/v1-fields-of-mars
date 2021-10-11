use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigBase<T> {
    pub anchor_token: T,
    pub staking_token: T,
}

pub type InstantiateMsg = ConfigBase<String>;
