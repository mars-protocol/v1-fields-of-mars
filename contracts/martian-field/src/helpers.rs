use cosmwasm_std::{
    Coin, Decimal, Env, QuerierWrapper, Reply, StdError, StdResult, SubMsgExecutionResponse,
    Uint128,
};

use cw_asset::{Asset, AssetList};

use fields_of_mars::martian_field::{Config, Health, Position, State};

/// Extract response from reply
pub fn unwrap_reply(reply: Reply) -> StdResult<SubMsgExecutionResponse> {
    reply.result.into_result().map_err(|e| StdError::generic_err(e))
}

/// Assert that fund of exactly the same type and amount was sent along with a message
pub fn assert_sent_fund(expected: &Asset, received: &[Coin]) -> StdResult<()> {
    let list = AssetList::from(received);

    let received_amount = if let Some(received_asset) = list.find(&expected.info) {
        received_asset.amount
    } else {
        Uint128::zero()
    };

    if received_amount == expected.amount {
        Ok(())
    } else {
        Err(StdError::generic_err(
            format!("sent fund mismatch! expected: {}, received {}", expected, received_amount)
        ))
    }
}

/// Compute the health of a user's position
pub fn compute_health(
    querier: &QuerierWrapper,
    env: &Env,
    config: &Config,
    state: &State,
    position: &Position,
) -> StdResult<Health> {
    // Query information necessary for computing values and LTV:
    // 1. bond
    // 2. debt
    // 3. pair
    // 4. price; NOTE: Price of the primary asset is quoted in the secondary asset, not in UST or USD
    let (total_bonded_amount, _) =
        config.staking.query_reward_info(querier, &env.contract.address, env.block.height)?;

    let total_debt_amount = config.red_bank.query_user_debt(
        querier,
        &env.contract.address,
        &config.secondary_asset_info,
    )?;

    let (primary_asset_depth, secondary_asset_depth, total_shares) = config.pair.query_pool(
        querier,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;

    let primary_asset_price = config.oracle.query_price(querier, &config.primary_asset_info)?;

    // Compute value of the user's bonded shares
    // NOTE:
    // 1. Value is denominated in the secondary asset, not in UST or USD
    // 2. Must handle the case where total_bond_units = 0
    let bond_value = if state.total_bond_units.is_zero() {
        Uint128::zero()
    } else {
        let total_pool_value = primary_asset_depth * primary_asset_price + secondary_asset_depth;
        let total_bond_value = total_pool_value.multiply_ratio(total_bonded_amount, total_shares);
        total_bond_value.multiply_ratio(position.bond_units, state.total_bond_units)
    };

    // Compute value of the user's debt
    // NOTE:
    // 1. Debt is denominated in the secondary asset, so we don't need to multiply a price here
    // 2. Must handle the case where total_debt_units = 0
    let debt_value = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_debt_amount.multiply_ratio(position.debt_units, state.total_debt_units)
    };

    // Compute LTV
    // NOTE: Must handle division by zero!
    let ltv = if bond_value.is_zero() {
        None
    } else {
        Some(Decimal::from_ratio(debt_value, bond_value))
    };

    Ok(Health {
        bond_value,
        debt_value,
        ltv,
    })
}
