use cosmwasm_std::{
    Api, Decimal, Extern, HumanAddr, MessageInfo, Querier, StdResult, Storage, Uint128,
};
use std::str::FromStr;
use terra_cosmwasm::TerraQuerier;

use fields_of_mars::asset::AssetInfo;

use crate::state::{read_config, read_position, read_state, Position};

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);
static COMMISSION_RATE: &str = "0.003";

/**
 * @notice Query necessary data, then calculate the user's loan-to-value ratio (LTV).
 *
 * @return asset_value: Uint128
 * @return debt_value: Uint128
 * @return ltv: Decimal
 *
 * E.g. User has $150 worth of asset and $100 worth of debt, then
 *      debt_ratio = 100 / 150 = 0.666...667
 */
pub fn compute_ltv<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: Option<HumanAddr>,
) -> StdResult<(Uint128, Uint128, Option<Decimal>)> {
    let config = read_config(&deps.storage)?;
    let state = read_state(&deps.storage)?;
    let strategy = deps.api.human_address(&state.strategy)?;
    let swap = config.swap.to_normal(deps)?;
    let staking = config.staking.to_normal(deps)?;
    let red_bank = config.red_bank.to_normal(deps)?;

    // If `user` is provided, calculate debt ratio of the user; if not, calculate the
    // overall debt ratio of the strategy.
    let position = if let Some(user) = user {
        read_position(&deps.storage, &deps.api.canonical_address(&user)?)?
    } else {
        Position {
            bond_units: state.total_bond_units,
            debt_units: state.total_debt_units,
            unbonded_ust_amount: Uint128::zero(),
            unbonded_asset_amount: Uint128::zero(),
        }
    };

    // Query data necessary for calculating the user's debt ratio
    let pool_info = swap.query_pool(
        deps,
        &AssetInfo::Token {
            contract_addr: deps.api.human_address(&config.asset_token)?,
        },
        &AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    )?;
    let pool_ust = pool_info.short_depth;
    let pool_token_supply = pool_info.share_supply;
    let total_debt_amount = red_bank.query_debt(&deps, &strategy)?;
    let total_bond_amount = staking.query_bond_amount(&deps, &strategy)?;

    // UST value of each LP token
    // Note: Here we don't check whether `pool_token_supply` is zero here because in
    // practice it should always be greater than zero
    let value_per_pool_token =
        Decimal::from_ratio(2 * pool_ust.u128(), pool_token_supply);

    // Amount of bonded LP tokens assigned to the user
    // Note: must handle division by zero!
    let bond_amount = if state.total_bond_units.is_zero() {
        Uint128::zero()
    } else {
        total_bond_amount.multiply_ratio(position.bond_units, state.total_bond_units)
    };

    // UST value of bonded LP tokens assigned to the user
    let bond_value = value_per_pool_token * bond_amount;

    // Value of borrowed UST assigned to the user
    // Note: must handle division by zero!
    let debt_value = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_debt_amount.multiply_ratio(position.debt_units, state.total_debt_units)
    };

    // Loan-to-value ratio
    // None if the user doesn't have any bonded asset (in which case LTV would be infinite)
    let utilization = if bond_value.is_zero() {
        None
    } else {
        Some(Decimal::from_ratio(debt_value, bond_value))
    };

    Ok((bond_value, debt_value, utilization))
}

/**
 * @dev Calculate the return amount, after commission and tax, when swapping a CW20 to UST
 * on Terraswap.
 *
 * Logic here is borrowed from
 * Commission: terraswap/terraswap/contracts/terraswap_pair/src/contract.rs#L525
 * Tax: terraswap/terraswap/packages/terraswap/src/asset.rs#L32
 *
 * Note: COMMISSION_RATE = 0.003
 */
pub fn compute_swap_return_amount<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    offer_amount: Uint128,
    offer_pool: Uint128,
    ask_pool: Uint128,
) -> StdResult<Uint128> {
    let cp = Uint128(offer_pool.u128() * ask_pool.u128());
    let return_amount = (ask_pool - cp.multiply_ratio(1u128, offer_pool + offer_amount))?;
    let commission = return_amount * Decimal::from_str(&COMMISSION_RATE).unwrap();
    deduct_tax(deps, (return_amount - commission)?, "uusd")
}

/**
 * @dev Given a total amount of UST, find the deviverable amount, after tax, if the amount
 * is to be transferred.
 * @param amount The total amount
 *
 * Forked from
 * https://github.com/terraswap/terraswap/blob/master/packages/terraswap/src/asset.rs#L58
 */
pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    amount: Uint128,
    denom: &str,
) -> StdResult<Uint128> {
    let tax = if denom == "uluna" {
        Uint128::zero()
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate = terra_querier.query_tax_rate()?.rate;
        let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;
        std::cmp::min(
            (amount
                - amount.multiply_ratio(
                    DECIMAL_FRACTION,
                    DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                ))?,
            tax_cap,
        )
    };
    amount - tax
}

/**
 * @notice Given a intended deliverable amount, find the total amount, including tax,
 * necessary for deliver this amount. Opposite operation of `deductTax`.
 * @param amount The intended deliverable amount
 */
pub fn add_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    amount: Uint128,
    denom: &str,
) -> StdResult<Uint128> {
    let tax = if denom == "luna" {
        Uint128::zero()
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate = terra_querier.query_tax_rate()?.rate;
        let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;
        std::cmp::min(amount * tax_rate, tax_cap)
    };
    Ok(amount + tax)
}

/**
 * @dev Find the amount of native tokens sent along with a message
 */
pub fn parse_ust_received(message: &MessageInfo) -> Uint128 {
    match message.sent_funds.iter().find(|fund| fund.denom == "uusd") {
        Some(coin) => coin.amount,
        None => Uint128::zero(),
    }
}
