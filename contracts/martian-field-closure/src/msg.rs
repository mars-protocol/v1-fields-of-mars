use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Refund the first 10 users of their assets; call this function repeatedly to refund all users
    Refund {},
    /// Once all positions have been closed, call this function to completely wipe contract storage
    PurgeStorage {},
}
