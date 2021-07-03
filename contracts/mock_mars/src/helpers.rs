use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{Api, Coin, Extern, Querier, StdResult, Storage, Uint128};
use terra_cosmwasm::TerraQuerier;

static DECIMAL_FRACTION: Uint128 = Uint128(1_000_000_000_000_000_000u128);

pub fn get_denom_amount_from_coins(coins: &[Coin], denom: &str) -> Uint256 {
    coins
        .iter()
        .find(|c| c.denom == denom)
        .map(|c| Uint256::from(c.amount))
        .unwrap_or_else(Uint256::zero)
}

pub fn deduct_tax<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    amount: Uint128,
    denom: &str,
) -> StdResult<Uint128> {
    let tax = if denom == "uluna" {
        Ok(Uint128::zero())
    } else {
        let terra_querier = TerraQuerier::new(&deps.querier);
        let tax_rate = terra_querier.query_tax_rate()?.rate;
        let tax_cap = terra_querier.query_tax_cap(denom.to_string())?.cap;

        Ok(std::cmp::min(
            (amount
                - amount.multiply_ratio(
                    DECIMAL_FRACTION,
                    DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                ))?,
            tax_cap,
        ))
    };

    amount - tax?
}
