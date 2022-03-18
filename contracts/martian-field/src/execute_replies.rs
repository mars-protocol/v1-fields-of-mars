use cosmwasm_std::{DepsMut, Response, StdResult, SubMsgExecutionResponse};

use cw_asset::{Asset, AssetList};

use fields_of_mars::adapters::Pair;

use crate::state::{Position, State, CACHED_USER_ADDR, CONFIG, POSITION, STATE};

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

    // parse event log to find the amount of liquidity tokens minted
    let minted_amount = Pair::parse_provide_events(&response.events)?;
    assets.add(&Asset::cw20(config.primary_pair.liquidity_token, minted_amount))?;

    // save the updated state/position
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    // finally, clear cached data
    CACHED_USER_ADDR.remove(deps.storage);

    // `shares_minted` should really be `liquidity_token_minted` according to my naming convention,
    // but it's a bit too long and doesn't look very good on Terra Finder's UI, so I opt for a shorter one
    Ok(Response::new()
        .add_attribute("action", "martian_field/reply/after_provide_liquidity")
        .add_attribute("shares_minted", minted_amount))
}

pub fn after_withdraw_liquidity(
    deps: DepsMut,
    response: SubMsgExecutionResponse,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let user_addr = CACHED_USER_ADDR.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // parse event log to find the amounts of assets returned
    let (primary_asset_withdrawn, secondary_asset_withdrawn) = Pair::parse_withdraw_events(
        &response.events,
        &config.primary_asset_info,
        &config.secondary_asset_info,
    )?;

    position.unlocked_assets.add(&primary_asset_withdrawn)?;
    position.unlocked_assets.add(&secondary_asset_withdrawn)?;

    POSITION.save(deps.storage, &user_addr, &position)?;
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field/reply/after_withdraw_liquidity")
        .add_attribute("user", user_addr)
        .add_attribute("primary_withdrawn", primary_asset_withdrawn.amount)
        .add_attribute("secondary_withdrawn", secondary_asset_withdrawn.amount))
}

pub fn after_swap(deps: DepsMut, response: SubMsgExecutionResponse) -> StdResult<Response> {
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
    let returned_asset_unchecked = Pair::parse_swap_events(&response.events)?;
    let returned_asset = returned_asset_unchecked.check(deps.api, None)?;
    assets.add(&returned_asset)?;

    // save the updated state/position
    if let Some(user_addr) = &user_addr_option {
        POSITION.save(deps.storage, user_addr, &position)?;
    } else {
        STATE.save(deps.storage, &state)?;
    }

    // finally, clear cached data
    CACHED_USER_ADDR.remove(deps.storage);

    Ok(Response::new()
        .add_attribute("action", "martian_field/reply/after_swap")
        .add_attribute("returned_asset", returned_asset.to_string()))
}
