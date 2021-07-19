use cosmwasm_std::{
    Api, CanonicalAddr, Decimal, Extern, Querier, StdResult, Storage, Uint128,
};

use fields_of_mars::martian_field::HealthResponse;

use crate::state::{Config, Position, State};

/// @notice Compute the contract's overall health
pub fn compute_state_health<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<HealthResponse> {
    let state = State::read(&deps.storage)?;
    _compute_health(&deps, state.total_bond_units, state.total_debt_units)
}

/// @notice Compute a position's health
pub fn compute_position_health<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: &CanonicalAddr,
) -> StdResult<HealthResponse> {
    let position = Position::read(&deps.storage, &user)?;
    _compute_health(&deps, position.bond_units, position.debt_units)
}

/// @notice Compute health info for given bond and debt units
fn _compute_health<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    bond_units: Uint128,
    debt_units: Uint128,
) -> StdResult<HealthResponse> {
    let config = Config::read_normal(deps)?;
    let state = State::read(&deps.storage)?;
    let contract_addr = deps.api.human_address(&state.contract_addr)?;

    // Info of the TerraSwap pool
    let pool_info =
        config.swap.query_pool(&deps, &config.long_asset, &config.short_asset)?;

    // Total amount of debt owed to Mars
    let total_debt =
        config.red_bank.query_debt(&deps, &contract_addr, &config.short_asset)?;

    // Total amount of share tokens bonded in the staking contract
    let total_bond = config.staking.query_bond(&deps, &contract_addr)?;

    // Value of each units of share, measured in the short asset
    // Note: Here we don't check whether `pool_info.share_supply` is zero here because
    // in practice it should never be zero
    let share_value = Decimal::from_ratio(
        pool_info.short_depth + pool_info.short_depth,
        pool_info.share_supply,
    );

    // Amount of bonded shares assigned to the user
    // Note: must handle division by zero!
    let bond_amount = if state.total_bond_units.is_zero() {
        Uint128::zero()
    } else {
        total_bond.multiply_ratio(bond_units, state.total_bond_units)
    };

    // Value of bonded shares assigned to the user
    let bond_value = bond_amount * share_value;

    // Value of debt assigned to the user
    // Note: must handle division by zero!
    let debt_value = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_debt.multiply_ratio(debt_units, state.total_debt_units)
    };

    // Loan-to-value ratio
    // Note: must handle division by zero!
    // `bond_units` can be zero if the position has been closed, pending liquidation
    let ltv = if bond_value.is_zero() {
        None
    } else {
        Some(Decimal::from_ratio(debt_value, bond_value))
    };

    Ok(HealthResponse {
        bond_value,
        debt_value,
        ltv,
    })
}
