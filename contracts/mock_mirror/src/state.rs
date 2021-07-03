use cosmwasm_std::{CanonicalAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static KEY_CONFIG: &[u8] = b"config";
static PREFIX_REWARD_INFO: &[u8] = b"reward_info";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub mirror_token: CanonicalAddr,
    pub asset_token: CanonicalAddr,
    pub staking_token: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct RewardInfo {
    pub bond_amount: Uint128,
}

//----------------------------------------------------------------------------------------
// READ/WRITE FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn read_config<S: Storage>(storage: &S) -> StdResult<Config> {
    singleton_read(storage, KEY_CONFIG).load()
}

pub fn write_config<S: Storage>(storage: &mut S, config: &Config) -> StdResult<()> {
    singleton(storage, KEY_CONFIG).save(config)
}

/**
 * @dev Return zero if the user's record is not found, instead of throwing an error.
 */
pub fn read_reward_info<S: Storage>(
    storage: &S,
    staker: &CanonicalAddr,
) -> StdResult<RewardInfo> {
    match bucket_read(PREFIX_REWARD_INFO, storage).may_load(staker.as_slice())? {
        Some(reward_info) => Ok(reward_info),
        None => Ok(RewardInfo {
            bond_amount: Uint128(0),
        }),
    }
}

pub fn write_reward_info<S: Storage>(
    storage: &mut S,
    staker: &CanonicalAddr,
    reward_info: &RewardInfo,
) -> StdResult<()> {
    bucket(PREFIX_REWARD_INFO, storage).save(staker.as_slice(), reward_info)
}
