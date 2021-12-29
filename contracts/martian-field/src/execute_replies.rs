use cosmwasm_std::{DepsMut, Response, StdResult, SubMsgExecutionResponse};

use cw_asset::{Asset, AssetList};

use fields_of_mars::adapters::Pair;
use fields_of_mars::martian_field::{Position, State};

use crate::state::{CACHED_USER_ADDR, CONFIG, POSITION, STATE};

pub fn after_provide_liquidity(
    deps: DepsMut,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // if this is a user providing their unlocked assets, the user's address should have been cached
    // if this is a reward harvesting operation, no user address should have been cached. `may_load`
    // should return `None` in this case
    let user_addr_option = CACHED_USER_ADDR.may_load(deps.storage)?;

    // if a user address is cached, we update the user's unlocked assets
    // if not, we update the state's pending rewards
    let mut state = State::default();
    let mut position = Position::default();
    let assets: &mut AssetList;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    let liquidity_token_minted_amount = Pair::parse_provide_events(&response.events)?;
    let liquidity_token_minted =
        Asset::cw20(config.pair.liquidity_token.clone(), liquidity_token_minted_amount);
    assets.add(&liquidity_token_minted)?;

    // save the updated state/position
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    // finally, clear cached data
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_provide_liquidity")
        .add_attribute("liquidity_token_minted", liquidity_token_minted.amount))
}

pub fn after_withdraw_liquidity(
    deps: DepsMut,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = CACHED_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    let (primary_asset_withdrawn, secondary_asset_withdrawn) = Pair::parse_withdraw_events(
        &response.events,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;

    // The withdrawn amounts returned in Astroport's response event are the pre-tax amounts. We need
    // to deduct tax to find the amounts we actually received. We add the after-tax amounts to the
    // user's unlocked assets
    let mut primary_asset_to_add = primary_asset_withdrawn.clone();
    primary_asset_to_add.deduct_tax(&deps.querier)?;
    let mut secondary_asset_to_add = secondary_asset_withdrawn.clone();
    secondary_asset_to_add.deduct_tax(&deps.querier)?;

    position.unlocked_assets.add(&primary_asset_to_add)?;
    position.unlocked_assets.add(&secondary_asset_to_add)?;

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

    // if this is a user swapping their unlocked assets, the user's address should have been cached
    // if this is a reward harvesting operation, no user address should have been cached. `may_load`
    // should return `None` in this case
    let user_addr_option = CACHED_USER_ADDR.may_load(deps.storage)?;

    // if a user address is cached, we update the user's unlocked assets
    // if not, we update the state's pending rewards
    let mut state = State::default();
    let mut position = Position::default();
    let assets: &mut AssetList;
    if let Some(user_addr) = &user_addr_option {
        position = POSITION.load(deps.storage, user_addr).unwrap_or_default();
        assets = &mut position.unlocked_assets;
    } else {
        state = STATE.load(deps.storage)?;
        assets = &mut state.pending_rewards;
    }

    // parse Astroport's event log to find out how much asset was returned from the swap
    let secondary_asset_returned_amount = Pair::parse_swap_events(&response.events)?;
    let secondary_asset_returned =
        Asset::new(config.secondary_asset_info.clone(), secondary_asset_returned_amount);

    // the return amount returned in Astroport's response event is the pre-tax amount. we need to
    // deduct tax to find the amount we actually received. we add the after-tax amount to the user's
    // unlocked asset
    let mut secondary_asset_to_add = secondary_asset_returned.clone();
    secondary_asset_to_add.deduct_tax(&deps.querier)?;
    assets.add(&secondary_asset_to_add)?;

    // save the updated state/position
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    // finally, clear cached data
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field :: reply :: after_swap")
        .add_attribute("secondary_returned_amount", secondary_asset_returned.amount)
        .add_attribute("secondary_added_amount", secondary_asset_to_add.amount))
}
