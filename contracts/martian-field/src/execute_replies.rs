use cosmwasm_std::{DepsMut, Response, StdResult, SubMsgExecutionResponse};

use fields_of_mars::adapters::{Asset, Pair};

use crate::helpers::add_unlocked_asset;
use crate::state::{CACHED_USER_ADDR, CONFIG, POSITION};

pub fn after_provide_liquidity(
    deps: DepsMut,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = CACHED_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let share_minted_amount = Pair::parse_provide_events(&response.events)?;
    let shares_to_add = Asset::cw20(&config.pair.liquidity_token, share_minted_amount);

    add_unlocked_asset(&mut position, &shares_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_provide_liquidity")
        .add_attribute("user_addr", user_addr)
        .add_attribute("share_added_amount", shares_to_add.amount))
}

pub fn after_withdraw_liquidity(
    deps: DepsMut,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = CACHED_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let (primary_asset_withdrawn, secondary_asset_withdrawn) = Pair::parse_withdraw_events(
        &response.events,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;

    // The withdrawn amounts returned in Astroport's response event are the pre-tax amounts. We need
    // to deduct tax to find the amounts we actually received. We add the after-tax amounts to the
    // user's unlocked assets
    let primary_asset_to_add = primary_asset_withdrawn.deduct_tax(&deps.querier)?;
    let secondary_asset_to_add = secondary_asset_withdrawn.deduct_tax(&deps.querier)?;

    add_unlocked_asset(&mut position, &primary_asset_to_add);
    add_unlocked_asset(&mut position, &secondary_asset_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_withdraw_liquidity")
        .add_attribute("user_addr", user_addr)
        .add_attribute("primary_withdrawn_amount", primary_asset_withdrawn.amount)
        .add_attribute("primary_added_amount", primary_asset_to_add.amount)
        .add_attribute("secondary_withdrawn_amount", secondary_asset_withdrawn.amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}

pub fn after_swap(deps: DepsMut, response: SubMsgExecutionResponse) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = CACHED_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    let secondary_asset_returned_amount = Pair::parse_swap_events(&response.events)?;
    let secondary_asset_returned =
        Asset::new(&config.secondary_asset_info, secondary_asset_returned_amount);

    // The return amount returned in Astroport's response event is the pre-tax amount. We need to
    // deduct tax to find the amount we actually received. We add the after-tax amount to the user's
    // unlocked asset
    let secondary_asset_to_add = secondary_asset_returned.deduct_tax(&deps.querier)?;

    add_unlocked_asset(&mut position, &secondary_asset_to_add);

    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_swap")
        .add_attribute("user_addr", user_addr)
        .add_attribute("secondary_returned_amount", secondary_asset_returned.amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}
