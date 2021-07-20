use cosmwasm_std::{CanonicalAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static KEY_CONFIG: &[u8] = b"config";
static PREFIX_STAKER_INFO: &[u8] = b"staker_info";

//----------------------------------------------------------------------------------------
// Storage Types
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub anchor_token: CanonicalAddr,
    pub staking_token: CanonicalAddr,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub bond_amount: Uint128,
}

//----------------------------------------------------------------------------------------
// Read/write functions
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
pub fn read_staker_info<S: Storage>(
    storage: &S,
    staker: &CanonicalAddr,
) -> StdResult<StakerInfo> {
    match bucket_read(PREFIX_STAKER_INFO, storage).may_load(staker.as_slice())? {
        Some(staker_info) => Ok(staker_info),
        None => Ok(StakerInfo {
            bond_amount: Uint128(0),
        }),
    }
}

pub fn write_staker_info<S: Storage>(
    storage: &mut S,
    staker: &CanonicalAddr,
    staker_info: &StakerInfo,
) -> StdResult<()> {
    bucket(PREFIX_STAKER_INFO, storage).save(staker.as_slice(), staker_info)
}
