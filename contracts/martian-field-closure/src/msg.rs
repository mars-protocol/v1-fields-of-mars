use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Withdraw assets from Astroport, swap all remaining Astro tokens to UST
    Unwind {},
    /// Refund the first 10 users of their assets; call this function repeatedly to refund all users
    Refund {},
    /// Once the previous steps have been completed, call this function to completely wipe contract storage
    PurgeStorage {},
}
