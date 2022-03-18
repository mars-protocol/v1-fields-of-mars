use cosmwasm_std::{Addr, Deps, DepsMut, Env, Response, StdResult};
use cosmwasm_std::{Decimal, Uint128};
use cw_asset::AssetListUnchecked;
use cw_storage_plus::Map;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::queries::query_position;

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyPositionResponse {
    /// Amount of bond units representing user's share of bonded LP tokens
    pub bond_units: Uint128,
    /// Amount of debt units representing user's share of the debt
    pub debt_units: Uint128,
    /// Amount of assets not locked in Astroport pool; pending refund or liquidation
    pub unlocked_assets: AssetListUnchecked,
}

#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct LegacyHealthResponse {
    /// Amount of primary pair liquidity tokens owned by this position
    pub bond_amount: Uint128,
    /// Value of the position's asset, measured in the short asset
    pub bond_value: Uint128,
    /// Amount of secondary assets owed by this position
    pub debt_amount: Uint128,
    /// Value of the position's debt, measured in the short asset
    pub debt_value: Uint128,
    /// The ratio of `debt_value` to `bond_value`; None if `bond_value` is zero
    pub ltv: Option<Decimal>,
}

/// Every time the user invokes `update_position`, we record a snaphot of the position
///
/// This snapshot does have any impact on the contract's normal functioning. Rather it is used by
/// the frontend to calculate PnL. Once we have the infrastructure for calculating PnL off-chain
/// available, we will migrate the contract to delete this
#[derive(Default, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Snapshot {
    pub time: u64,
    pub height: u64,
    pub position: LegacyPositionResponse,
    pub health: LegacyHealthResponse,
}

// snapshot is used by the frontend calculate user PnL. once we build a transaction indexer that can
// calculate PnL without relying on on-chain snapshots, this will be removed
pub const SNAPSHOT: Map<&Addr, Snapshot> = Map::new("snapshot");

pub fn record_snapshot(deps: DepsMut, env: Env, user_addr: Addr) -> StdResult<Response> {
    let time = env.block.time.seconds();
    let height = env.block.height;
    let position = query_position(deps.as_ref(), env, user_addr.clone())?;

    let legacy_position = LegacyPositionResponse {
        bond_units: position.bond_units,
        debt_units: position.debt_units,
        unlocked_assets: position.unlocked_assets,
    };
    let legacy_health = LegacyHealthResponse {
        bond_amount: position.bond_amount,
        bond_value: position.bond_value,
        debt_amount: position.debt_amount,
        debt_value: position.debt_value,
        ltv: position.ltv,
    };
    let snapshot = Snapshot {
        time,
        height,
        position: legacy_position,
        health: legacy_health,
    };

    SNAPSHOT.save(deps.storage, &user_addr, &snapshot)?;

    Ok(Response::new().add_attribute("action", "martian_field/callback/snapshot"))
}

pub fn query_snapshot(deps: Deps, user: String) -> StdResult<Snapshot> {
    let user_addr = deps.api.addr_validate(&user)?;
    Ok(SNAPSHOT.load(deps.storage, &user_addr).unwrap_or_default())
}
