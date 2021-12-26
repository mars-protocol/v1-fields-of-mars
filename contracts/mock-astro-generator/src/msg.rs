use cosmwasm_std::Addr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    // Address of the Astroport liquidity token which is to be staked
    pub liquidity_token: Addr,
    // Address of ASTRO token, i.e. the reward token
    pub astro_token: Addr,
    // Optionally, address of the proxy reward token, e.g. for MIR-UST LP this would be the MIR token
    pub proxy_reward_token: Option<Addr>,
}

pub type InstantiateMsg = Config;
