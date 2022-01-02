use cosmwasm_std::{Deps, Env, StdResult};

use fields_of_mars::martian_field::{ConfigUnchecked, Health, PositionUnchecked, State};

use crate::health::compute_health;
use crate::state::{CONFIG, POSITION, STATE};

pub fn query_config(deps: Deps, _env: Env) -> StdResult<ConfigUnchecked> {
    let config = CONFIG.load(deps.storage)?;
    Ok(config.into())
}

pub fn query_state(deps: Deps, _env: Env) -> StdResult<State> {
    STATE.load(deps.storage)
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
