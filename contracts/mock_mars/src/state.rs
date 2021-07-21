use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{Bucket, ReadonlyBucket, ReadonlySingleton, Singleton};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static KEY_CONFIG: &[u8] = b"config";
pub static NAMESPACE_POSITION: &[u8] = b"users";

//----------------------------------------------------------------------------------------
// Config
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub mock_interest_rate: Decimal256,
}

impl Config {
    pub fn write(&self, storage: &mut dyn Storage) -> StdResult<()> {
        Singleton::new(storage, KEY_CONFIG).save(self)
    }

    pub fn read(storage: &dyn Storage) -> StdResult<Self> {
        ReadonlySingleton::new(storage, KEY_CONFIG).load()
    }
}

//----------------------------------------------------------------------------------------
// Position
//----------------------------------------------------------------------------------------

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

impl Position {
    pub fn write(
        &self,
        storage: &mut dyn Storage,
        denom: &str,
        user: &CanonicalAddr,
    ) -> StdResult<()> {
        Bucket::multilevel(storage, &[NAMESPACE_POSITION, denom.as_bytes()])
            .save(user.as_slice(), self)
    }

    pub fn read(
        storage: &dyn Storage,
        denom: &str,
        user: &CanonicalAddr,
    ) -> StdResult<Self> {
        ReadonlyBucket::multilevel(storage, &[NAMESPACE_POSITION, denom.as_bytes()])
            .load(user.as_slice())
    }
}
