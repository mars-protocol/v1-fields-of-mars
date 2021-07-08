use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::{singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static KEY_CONFIG: &[u8] = b"config";
static KEY_STATE: &[u8] = b"state";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub token_contract: CanonicalAddr,
    pub exchange_rate: Decimal,
    pub er_threshold: Decimal,
    pub peg_recovery_fee: Decimal,
    pub requested_with_fee: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub total_bond_amount: Uint128,
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

pub fn read_state<S: Storage>(storage: &S) -> StdResult<State> {
    singleton_read(storage, KEY_STATE).load()
}

pub fn write_state<S: Storage>(storage: &mut S, state: &State) -> StdResult<()> {
    singleton(storage, KEY_STATE).save(state)
}
