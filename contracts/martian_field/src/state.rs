use cosmwasm_std::{
    Api, CanonicalAddr, Decimal, Extern, Querier, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{bucket, bucket_read, singleton, singleton_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use fields_of_mars::{
    asset::{AssetInfoRaw, AssetRaw},
    martian_field::{
        ConfigResponse, HealthResponse, InitMsg, PositionResponse, SnapshotResponse,
        StateResponse,
    },
    red_bank::RedBankRaw,
    staking::StakingRaw,
    swap::SwapRaw,
};

pub static KEY_CONFIG: &[u8] = b"config";
pub static KEY_STATE: &[u8] = b"state";
pub static PREFIX_POSITION: &[u8] = b"position";
pub static PREFIX_SNAPSHOT: &[u8] = b"snapshot";

//----------------------------------------------------------------------------------------
// Config
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Info of the asset to be deposited by the user
    pub long_asset: AssetInfoRaw,
    /// Info of the asset to be either deposited by user or borrowed from Mars
    pub short_asset: AssetInfoRaw,
    /// Address of Mars liquidity pool aka Red Bank
    pub red_bank: RedBankRaw,
    /// TerraSwap/Astroport pair of long/short assets
    pub swap: SwapRaw,
    /// Staking contract where LP tokens can be bonded to earn rewards
    pub staking: StakingRaw,
    /// Accounts who can harvest
    pub keepers: Vec<CanonicalAddr>,
    /// Account to receive fee payments
    pub treasury: CanonicalAddr,
    /// Account who can update config
    pub governance: CanonicalAddr,
    /// Maximum loan-to-value ratio (LTV) above which a user can be liquidated
    pub max_ltv: Decimal,
    /// Percentage of profit to be charged as performance fee
    pub fee_rate: Decimal,
}

impl Config {
    pub fn from_init_msg<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
        msg: &InitMsg,
    ) -> StdResult<Self> {
        let keepers = msg
            .keepers
            .iter()
            .map(|keeper| deps.api.canonical_address(&keeper).unwrap())
            .collect();

        let config = Self {
            long_asset: msg.long_asset.to_raw(deps)?,
            short_asset: msg.short_asset.to_raw(deps)?,
            red_bank: msg.red_bank.to_raw(deps)?,
            swap: msg.swap.to_raw(deps)?,
            staking: msg.staking.to_raw(deps)?,
            keepers,
            treasury: deps.api.canonical_address(&msg.treasury)?,
            governance: deps.api.canonical_address(&msg.governance)?,
            max_ltv: msg.max_ltv,
            fee_rate: msg.fee_rate,
        };

        Ok(config)
    }

    pub fn to_response<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<ConfigResponse> {
        let keepers = self
            .keepers
            .iter()
            .map(|keeper| deps.api.human_address(&keeper).unwrap())
            .collect();

        let response = ConfigResponse {
            long_asset: self.long_asset.to_normal(deps)?,
            short_asset: self.short_asset.to_normal(deps)?,
            red_bank: self.red_bank.to_normal(deps)?,
            swap: self.swap.to_normal(deps)?,
            staking: self.staking.to_normal(deps)?,
            keepers,
            treasury: deps.api.human_address(&self.treasury)?,
            governance: deps.api.human_address(&self.governance)?,
            max_ltv: self.max_ltv,
            fee_rate: self.fee_rate,
        };

        Ok(response)
    }

    pub fn write<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(storage, KEY_CONFIG).save(self)
    }

    pub fn read<S: Storage>(storage: &S) -> StdResult<Self> {
        singleton_read(storage, KEY_CONFIG).load()
    }

    pub fn read_normal<S: Storage, A: Api, Q: Querier>(
        deps: &Extern<S, A, Q>,
    ) -> StdResult<ConfigResponse> {
        Self::read(&deps.storage)?.to_response(deps)
    }
}

//----------------------------------------------------------------------------------------
// State
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    /// Address of this strategy
    pub contract_addr: CanonicalAddr,
    /// Total amount of bond units; used to calculate each user's share of bonded LP tokens
    pub total_bond_units: Uint128,
    /// Total amount of debt units; used to calculate each user's share of the debt
    pub total_debt_units: Uint128,
}

impl State {
    pub fn new(contract_addr: &CanonicalAddr) -> Self {
        State {
            contract_addr: contract_addr.clone(),
            total_bond_units: Uint128::zero(),
            total_debt_units: Uint128::zero(),
        }
    }

    pub fn to_response(&self) -> StdResult<StateResponse> {
        Ok(StateResponse {
            total_bond_units: self.total_bond_units,
            total_debt_units: self.total_debt_units,
        })
    }

    pub fn write<S: Storage>(&self, storage: &mut S) -> StdResult<()> {
        singleton(storage, KEY_STATE).save(self)
    }

    pub fn read<S: Storage>(storage: &S) -> StdResult<Self> {
        singleton_read(storage, KEY_STATE).load()
    }
}

//----------------------------------------------------------------------------------------
// Position
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Position {
    /// Amount of asset units representing user's share of the bonded assets
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not provided to the pool or locked in the staking contract
    pub unlocked_assets: [AssetRaw; 3],
}

impl Position {
    pub fn new(config: &Config) -> Self {
        Position {
            bond_units: Uint128::zero(),
            debt_units: Uint128::zero(),
            unlocked_assets: [
                AssetRaw {
                    info: config.long_asset.clone(),
                    amount: Uint128::zero(),
                },
                AssetRaw {
                    info: config.short_asset.clone(),
                    amount: Uint128::zero(),
                },
                AssetRaw {
                    info: AssetInfoRaw::Token {
                        contract_addr: config.swap.share_token.clone(),
                    },
                    amount: Uint128::zero(),
                },
            ],
        }
    }

    pub fn to_response<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<PositionResponse> {
        Ok(PositionResponse {
            is_active: self.is_active(),
            bond_units: self.bond_units,
            debt_units: self.debt_units,
            unlocked_assets: [
                self.unlocked_assets[0].to_normal(&deps)?,
                self.unlocked_assets[1].to_normal(&deps)?,
                self.unlocked_assets[02].to_normal(&deps)?,
            ],
        })
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
            self.unlocked_assets[0].amount,
            self.unlocked_assets[1].amount,
        ]
        .iter()
        .all(|x| x.is_zero())
    }

    pub fn write<S: Storage>(
        &self,
        storage: &mut S,
        user: &CanonicalAddr,
    ) -> StdResult<()> {
        bucket(PREFIX_POSITION, storage).save(user.as_slice(), self)
    }

    pub fn read<S: Storage>(storage: &S, user: &CanonicalAddr) -> StdResult<Self> {
        bucket_read(PREFIX_POSITION, storage).load(user.as_slice())
    }

    /// @notice If the position doesn't exist, we initialize a new one
    pub fn read_or_new<S: Storage>(storage: &S, user: &CanonicalAddr) -> StdResult<Self> {
        match bucket_read(PREFIX_POSITION, storage).may_load(user.as_slice()) {
            Ok(Some(position)) => Ok(position),
            Ok(None) => Ok(Self::new(&Config::read(storage)?)),
            Err(err) => Err(err),
        }
    }

    pub fn delete<S: Storage>(storage: &mut S, user: &CanonicalAddr) {
        bucket::<_, Self>(PREFIX_POSITION, storage).remove(user.as_slice());
    }
}

//----------------------------------------------------------------------------------------
// Snapshot
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Snapshot {
    /// UNIX timestamp at which the snapshot was taken
    pub time: u64,
    /// Block number at which the snapshot was taken
    pub height: u64,
    /// Snapshot of the position's health info
    pub health: HealthResponse,
    /// Snapshot of the position
    pub position: Position,
}

impl Snapshot {
    pub fn to_response<S: Storage, A: Api, Q: Querier>(
        &self,
        deps: &Extern<S, A, Q>,
    ) -> StdResult<SnapshotResponse> {
        Ok(SnapshotResponse {
            time: self.time,
            height: self.height,
            health: self.health.clone(),
            position: self.position.to_response(&deps)?,
        })
    }

    pub fn write<S: Storage>(
        &self,
        storage: &mut S,
        user: &CanonicalAddr,
    ) -> StdResult<()> {
        bucket(PREFIX_SNAPSHOT, storage).save(user.as_slice(), self)
    }

    pub fn read<S: Storage>(storage: &S, user: &CanonicalAddr) -> StdResult<Self> {
        bucket_read(PREFIX_SNAPSHOT, storage).load(user.as_slice())
    }

    pub fn delete<S: Storage>(storage: &mut S, user: &CanonicalAddr) {
        bucket::<_, Self>(PREFIX_SNAPSHOT, storage).remove(user.as_slice());
    }
}
