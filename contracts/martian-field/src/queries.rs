use cosmwasm_std::{Deps, Env, Order, StdResult};
use cw_storage_plus::Bound;

use fields_of_mars::martian_field::msg::PositionsResponseItem;
use fields_of_mars::martian_field::{ConfigUnchecked, Health, PositionUnchecked, Snapshot, State};

use crate::health::compute_health;
use crate::state::{CONFIG, POSITION, SNAPSHOT, STATE};

// Default settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_config(deps: Deps, _env: Env) -> StdResult<ConfigUnchecked> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.into())
}

pub fn query_state(deps: Deps, _env: Env) -> StdResult<State> {
    STATE.load(deps.storage)
}

pub fn query_positions(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<PositionsResponseItem>> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    POSITION
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, v) = item?;
            Ok(PositionsResponseItem {
                user: String::from_utf8(k)?,
                position: v.into(),
            })
        })
        .collect()
}

pub fn query_position(deps: Deps, _env: Env, user: String) -> StdResult<PositionUnchecked> {
    let user_addr = deps.api.addr_validate(&user)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    Ok(position.into())
}

pub fn query_health(deps: Deps, env: Env, user: String) -> StdResult<Health> {
    let user_addr = deps.api.addr_validate(&user)?;
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();
    compute_health(&deps.querier, &env, &config, &state, &position)
}

pub fn query_snapshot(deps: Deps, user: String) -> StdResult<Snapshot> {
    let user_addr = deps.api.addr_validate(&user)?;
    Ok(SNAPSHOT.load(deps.storage, &user_addr).unwrap_or_default())
}
