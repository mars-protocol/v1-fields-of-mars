use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{singleton, singleton_read, Bucket, ReadonlyBucket};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static KEY_CONFIG: &[u8] = b"config";
pub static PREFIX_POSITION: &[u8] = b"users";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub mock_interest_rate: Decimal256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub borrowed_amount: Uint256,
}

impl Default for Position {
    fn default() -> Self {
        Position {
            borrowed_amount: Uint256::zero(),
        }
    }
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

pub fn read_position<S: Storage>(
    storage: &S,
    user: &CanonicalAddr,
    asset: &str,
) -> StdResult<Position> {
    ReadonlyBucket::multilevel(&[PREFIX_POSITION, asset.as_bytes()], storage)
        .load(user.as_slice())
}

pub fn write_position<S: Storage>(
    storage: &mut S,
    user: &CanonicalAddr,
    asset: &str,
    position: &Position,
) -> StdResult<()> {
    Bucket::multilevel(&[PREFIX_POSITION, asset.as_bytes()], storage)
        .save(user.as_slice(), position)
}
