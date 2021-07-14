use cosmwasm_std::{CanonicalAddr, Decimal, StdResult, Storage, Uint128};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static KEY_CONFIG: &[u8] = b"config";
pub static KEY_STATE: &[u8] = b"state";
pub static PREFIX_POSITION: &[u8] = b"positions";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Account who can update config
    pub owner: CanonicalAddr,
    /// Accounts who can harvest
    pub operators: Vec<CanonicalAddr>,
    /// Address of the protocol treasury to receive fees payments
    pub treasury: CanonicalAddr,
    /// Address of the token to be deposited by users (MIR, mAsset, ANC)
    pub asset_token: CanonicalAddr,
    /// Address of the token that is to be harvested as rewards (MIR, ANC)
    pub reward_token: CanonicalAddr,
    /// Address of the TerraSwap pair
    pub pool: CanonicalAddr,
    /// Address of the TerraSwap LP token
    pub pool_token: CanonicalAddr,
    /// Address of Mars liquidity pool
    pub mars: CanonicalAddr,
    /// Address of the staking contract
    pub staking_contract: CanonicalAddr,
    /// Type of the staking contract ("anchor" or "mirror")
    pub staking_type: String,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub performance_fee_rate: Decimal,
    /// Percentage of asset to be charged as liquidation fee
    pub liquidation_fee_rate: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Address of this strategy
    pub strategy: CanonicalAddr,
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
}

impl State {
    pub fn new(strategy: CanonicalAddr) -> Self {
        State {
            strategy,
            total_bond_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    /// Amount of asset units representing user's share of the bonded assets
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of unstaked UST in the user's position
    pub unbonded_ust_amount: Uint128,
    /// Amount of unstaked asset token in the user's position
    pub unbonded_asset_amount: Uint128,
}

impl Position {
    pub const fn default() -> Self {
        Position {
            bond_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unbonded_ust_amount: Uint128::zero(),
            unbonded_asset_amount: Uint128::zero(),
        }
    }

    pub fn is_active(&self) -> bool {
        if self.bond_units.is_zero() {
            false
        } else {
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        vec![
            self.bond_units,
            self.debt_units,
            self.unbonded_ust_amount,
            self.unbonded_asset_amount,
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
    bucket_read(PREFIX_POSITION, storage).load(account.as_slice())
}

pub fn write_position<S: Storage>(
    storage: &mut S,
    account: &CanonicalAddr,
    position: &Position,
) -> StdResult<()> {
    bucket(PREFIX_POSITION, storage).save(account.as_slice(), position)
}

pub fn delete_position<S: Storage>(storage: &mut S, account: &CanonicalAddr) {
    bucket::<_, Position>(PREFIX_POSITION, storage).remove(account.as_slice());
}
