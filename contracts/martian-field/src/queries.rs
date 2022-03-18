use cosmwasm_std::{Addr, Deps, Env, Order, QuerierWrapper, StdResult};
use cw_storage_plus::Bound;

use fields_of_mars::martian_field::{Config, ConfigUnchecked, PositionResponse, PositionsResponseItem};

use crate::health::compute_health;
use crate::state::{Position, State, CONFIG, POSITION, STATE};

// Default settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_config(deps: Deps) -> StdResult<ConfigUnchecked> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.into())
}

pub fn query_state(deps: Deps, env: Env) -> StdResult<PositionResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    
    _query_position(&deps.querier, &env, &config, &state, &state.clone().into())
}

pub fn query_positions(
    deps: Deps,
    env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<PositionsResponseItem>> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let start = start_after.map(Bound::exclusive);

    POSITION
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (k, v) = item?;
            Ok(PositionsResponseItem {
                user: String::from_utf8(k)?,
                position: _query_position(&deps.querier, &env, &config, &state, &v)?,
            })
        })
        .collect()
}

pub fn query_position(deps: Deps, env: Env, user_addr: Addr) -> StdResult<PositionResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    _query_position(&deps.querier, &env, &config, &state, &position)
}

/// Compute health, and compose the result into a `PositionResponse` object
fn _query_position(
    querier: &QuerierWrapper,
    env: &Env,
    config: &Config,
    state: &State,
    position: &Position,
) -> StdResult<PositionResponse> {
    let health = compute_health(querier, env, config, state, position)?;

    Ok(PositionResponse {
        bond_units: position.bond_units,
        bond_amount: health.bond_amount,
        bond_value: health.bond_value,
        debt_units: position.debt_units,
        debt_amount: health.debt_amount,
        debt_value: health.debt_value,
        ltv: health.ltv,
        unlocked_assets: position.unlocked_assets.clone().into(),
    })
}
