use cosmwasm_std::{CanonicalAddr, StdResult, Storage, Uint128};
use cosmwasm_storage::{Bucket, ReadonlyBucket, ReadonlySingleton, Singleton};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use field_of_mars::staking::anchor_staking::MockInstantiateMsg;

static KEY_CONFIG: &[u8] = b"config";
static NAMESPACE_POSITION: &[u8] = b"position";

//----------------------------------------------------------------------------------------
// Config
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config(pub MockInstantiateMsg);

impl Config {
    pub fn write(&self, storage: &mut dyn Storage) -> StdResult<()> {
        Singleton::new(storage, KEY_CONFIG).save(self)
    }

    pub fn read(storage: &dyn Storage) -> StdResult<MockInstantiateMsg> {
        Ok(ReadonlySingleton::<Self>::new(storage, KEY_CONFIG).load()?.0)
    }
}

//----------------------------------------------------------------------------------------
// Position
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    pub bond_amount: Uint128,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            bond_amount: Uint128::zero(),
        }
    }
}

impl Position {
    pub fn write(
        &self,
        storage: &mut dyn Storage,
        user: &CanonicalAddr,
    ) -> StdResult<()> {
        Bucket::new(storage, NAMESPACE_POSITION).save(user.as_slice(), self)
    }

    pub fn read(storage: &dyn Storage, user: &CanonicalAddr) -> StdResult<Self> {
        ReadonlyBucket::new(storage, NAMESPACE_POSITION).load(user.as_slice())
    }
}
