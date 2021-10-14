use cosmwasm_std::{Addr, Decimal, QuerierWrapper, StdError, StdResult, Uint128};

use fields_of_mars::adapters::{Asset, AssetInfo};
use fields_of_mars::martian_field::{Config, Health, Position, State};

pub fn find_unlocked_asset(position: &Position, asset_info: &AssetInfo) -> Asset {
    match position.unlocked_assets.iter().find(|asset| &asset.info == asset_info) {
        Some(asset) => asset.clone(),
        None => Asset::new(asset_info, Uint128::zero()),
    }
}

/// Given an array of assets, find the one that match given asset info, and increment the amount.
///
/// If not found, append the asset with given amount at the end of the array.
///
/// Return the amount after the increment.
pub fn add_unlocked_asset(position: &mut Position, asset_to_add: &Asset) -> Asset {
    match position.unlocked_assets.iter_mut().find(|asset| asset.info == asset_to_add.info) {
        Some(asset) => {
            asset.amount += asset_to_add.amount;
            asset.clone()
        }
        None => {
            position.unlocked_assets.push(asset_to_add.clone());
            asset_to_add.clone()
        }
    }
}

/// Same with `add_unlocked_asset` but reduce the amount instead
///
/// If the amount is reduced to zero, we remove the asset from the vector
pub fn deduct_unlocked_asset(position: &mut Position, asset_to_deduct: &Asset) -> StdResult<()> {
    match position.unlocked_assets.iter_mut().find(|asset| asset.info == asset_to_deduct.info) {
        Some(asset) => {
            asset.amount -= asset_to_deduct.amount;
        }
        None => {
            return Err(StdError::generic_err("cannot find asset to deduct"));
        }
    };
    position.unlocked_assets.retain(|asset| !asset.amount.is_zero());
    Ok(())
}

pub fn compute_health(
    querier: &QuerierWrapper,
    contract_addr: &Addr,
    config: &Config,
    state: &State,
    position: &Position,
) -> StdResult<Health> {
    // Query information necessary for computing values and LTV:
    // 1. bond
    // 2. debt
    // 3. pair
    // 4. price; NOTE: Price of the primary asset is quoted in the secondary asset, not in UST or USD
    let (total_bonded_amount, _) = config.staking.query_reward_info(querier, contract_addr)?;

    let total_debt_amount =
        config.red_bank.query_user_debt(querier, contract_addr, &config.secondary_asset_info)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use fields_of_mars::adapters::AssetInfo;
    use fields_of_mars::testing::{assert_eq_vec, assert_generic_error_message};

    #[test]
    fn test_add_unlocked_asset() {
        let mut position = Position::default();

        let primary_asset_info = AssetInfo::cw20(&Addr::unchecked("anchor_token"));
        let secondary_asset_info = AssetInfo::native(&"uusd");

        let asset = add_unlocked_asset(
            &mut position,
            &Asset::new(&primary_asset_info, Uint128::new(12345)),
        );

        assert_eq!(asset.amount, Uint128::new(12345));
        assert_eq_vec(
            position.unlocked_assets.clone(),
            vec![Asset::new(&primary_asset_info, 12345u128)],
        );

        let asset = add_unlocked_asset(
            &mut position,
            &Asset::new(&secondary_asset_info, Uint128::new(69420)),
        );

        assert_eq!(asset.amount, Uint128::new(69420));
        assert_eq_vec(
            position.unlocked_assets.clone(),
            vec![
                Asset::new(&primary_asset_info, 12345u128),
                Asset::new(&secondary_asset_info, 69420u128),
            ],
        );

        let asset = add_unlocked_asset(
            &mut position,
            &Asset::new(&primary_asset_info, Uint128::new(88888)),
        );

        assert_eq!(asset.amount, Uint128::new(101233)); // 12345 + 88888
        assert_eq_vec(
            position.unlocked_assets,
            vec![
                Asset::new(&primary_asset_info, 101233u128),
                Asset::new(&secondary_asset_info, 69420u128),
            ],
        );
    }

    #[test]
    fn test_deduct_unlocked_asset() {
        let mut position = Position::default();

        let primary_asset_info = AssetInfo::cw20(&Addr::unchecked("anchor_token"));
        let secondary_asset_info = AssetInfo::native(&"uusd");

        position.unlocked_assets.push(Asset::new(&primary_asset_info, 88888u128));

        let result = deduct_unlocked_asset(
            &mut position,
            &Asset::new(&secondary_asset_info, Uint128::new(69420)),
        );

        assert_generic_error_message(result, "cannot find asset to deduct");

        deduct_unlocked_asset(&mut position, &Asset::new(&primary_asset_info, Uint128::new(69420)))
            .unwrap();

        assert_eq_vec(
            position.unlocked_assets.clone(),
            vec![Asset::new(&primary_asset_info, 19468u128)],
        );

        deduct_unlocked_asset(&mut position, &Asset::new(&primary_asset_info, Uint128::new(19468)))
            .unwrap();

        assert_eq!(position.unlocked_assets.len(), 0); // assets with zero amount should have been removed
    }
}
