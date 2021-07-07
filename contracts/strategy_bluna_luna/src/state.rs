use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static KEY_CONFIG: &[u8] = b"config";
pub static KEY_STATE: &[u8] = b"state";
pub static PREFIX_POSITIION: &[u8] = b"positions";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account who can update config
    pub owner: CanonicalAddr,
    /// Address of the protocol treasury to receive fees payments
    pub treasury: CanonicalAddr,
    /// Address of bLUNA hub contract
    pub bluna_hub: CanonicalAddr,
    /// Address of the bLUNA token
    pub bluna_token: CanonicalAddr,
    /// Address of Terraswap bLUNA-LUNA pair
    pub pool: CanonicalAddr,
    /// Address of Terraswap LP token
    pub pool_token: CanonicalAddr,
    /// Address of Mars liquidity pool
    pub mars: CanonicalAddr,
    /// Percentage of asset to be charged as liquidation fee
    pub liquidation_fee_rate: Decimal,
    /// Maximum utilization above which a user can be liquidated
    pub liquidation_threshold: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Address of this strategy
    pub strategy: CanonicalAddr,
    /// Amount of pool units; each unit represents a share of the LP tokens held by the strategy
    pub total_pool_units: Uint128,
    /// Amount of debt units; each unit represents a share of the debt owed by the strategy to Mars
    pub total_debt_units: Uint128,
}

impl State {
    pub fn new(strategy: CanonicalAddr) -> Self {
        State {
            strategy,
            total_pool_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    /// Amount of pool units assigned to the user
    pub pool_units: Uint128,
    /// Amount of debt units assigned to the user
    pub debt_units: Uint128,
    /// Amount of LUNA not locked in Terraswap; pending refund or liquidation
    pub unlocked_luna_amount: Uint128,
    /// Amount of bLUNA not locked in Terraswap; pending refund or liquidation
    pub unlocked_bluna_amount: Uint128,
}

impl Position {
    pub const fn default() -> Self {
        Position {
            pool_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unlocked_luna_amount: Uint128::zero(),
            unlocked_bluna_amount: Uint128::zero(),
        }
    }

    pub fn is_active(&self) -> bool {
        if self.pool_units.is_zero() {
            false
        } else {
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        vec![
            self.pool_units,
            self.debt_units,
            self.unlocked_luna_amount,
            self.unlocked_bluna_amount,
        ]
        .iter()
        .all(|x| x.is_zero())
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

pub fn read_state<S: Storage>(storage: &S) -> StdResult<State> {
    singleton_read(storage, KEY_STATE).load()
}

pub fn write_state<S: Storage>(storage: &mut S, state: &State) -> StdResult<()> {
    singleton(storage, KEY_STATE).save(state)
}

pub fn read_position<S: Storage>(
    storage: &S,
    account: &CanonicalAddr,
) -> StdResult<Position> {
    bucket_read(PREFIX_POSITIION, storage).load(account.as_slice())
}

pub fn write_position<S: Storage>(
    storage: &mut S,
    account: &CanonicalAddr,
    position: &Position,
) -> StdResult<()> {
    bucket(PREFIX_POSITIION, storage).save(account.as_slice(), position)
}

pub fn delete_position<S: Storage>(storage: &mut S, account: &CanonicalAddr) {
    bucket::<_, Position>(PREFIX_POSITIION, storage).remove(account.as_slice());
}
