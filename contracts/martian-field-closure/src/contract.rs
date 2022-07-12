use cosmwasm_std::{
    entry_point, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, Event, MessageInfo, Order,
    Response, StdError, StdResult,
};
use cw_asset::Asset;

use crate::msg::ExecuteMsg;
use crate::state::{CONFIG, POSITION, STATE};

#[entry_point]
pub fn instantiate(_deps: DepsMut, _env: Env, _info: MessageInfo, _msg: Empty) -> StdResult<Response> {
    Err(StdError::generic_err("`instantiate` is not implemented"))
}

#[entry_point]
pub fn execute(deps: DepsMut, env: Env, _info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Refund {} => refund(deps, env),
    }
}

pub fn refund(deps: DepsMut, env: Env) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // query how much primary and secondary assets are available
    let mut primary_amount = config.primary_asset_info.query_balance(&deps.querier, &env.contract.address)?;
    let mut secondary_amount = config.secondary_asset_info.query_balance(&deps.querier, &env.contract.address)?;

    // find the first 10 user positions
    let user_units = POSITION
        .range(deps.storage, None, None, Order::Ascending)
        .take(10)
        .map(|item| {
            let (user_bytes, position) = item?;
            let user = String::from_utf8(user_bytes)?;
            Ok((user, position.bond_units))
        })
        .collect::<StdResult<Vec<_>>>()?;

    // refund the users
    // NOTE: when creating transfer msgs, must check whether the amount is >0
    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut events: Vec<Event> = vec![];
    for (user, bond_units) in user_units {
        let primary_refund_amount = primary_amount.multiply_ratio(bond_units, state.total_bond_units);
        if !primary_refund_amount.is_zero() {
            msgs.push(
                Asset::new(config.primary_asset_info.clone(), primary_refund_amount).transfer_msg(&user)?,
            );
        }

        let secondary_refund_amount = secondary_amount.multiply_ratio(bond_units, state.total_bond_units);
        if !secondary_refund_amount.is_zero() {
            msgs.push(
                Asset::new(config.secondary_asset_info.clone(), secondary_refund_amount).transfer_msg(&user)?,
            );
        }

        primary_amount -= primary_refund_amount;
        secondary_amount -= secondary_refund_amount;
        state.total_bond_units -= bond_units;

        events.push(
            Event::new("martian_field/refunded")
                .add_attribute("user", &user)
                .add_attribute("primary_refunded", primary_refund_amount)
                .add_attribute("secondary_refunded", secondary_refund_amount)
        );

        POSITION.remove(deps.storage, &Addr::unchecked(user));
    }

    STATE.save(deps.storage, &state)?;

    Ok(Response::new().add_messages(msgs).add_events(events))
}

pub fn purge_storage(deps: DepsMut) -> StdResult<Response> {
    let state = STATE.load(deps.storage)?;

    // can only purge when after positions have been refunded (total bond unit is zero)
    if !state.total_bond_units.is_zero() {
        return Err(StdError::generic_err("can only purge after all positions have been refunded"));
    }

    CONFIG.remove(deps.storage);
    STATE.remove(deps.storage);

    Ok(Response::new())
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, _msg: Empty) -> StdResult<Binary> {
    Err(StdError::generic_err("`query` is not implemented"))
}

#[entry_point]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: Empty) -> StdResult<Response> {
    // let config = CONFIG.load(deps.storage)?;

    // // if contract has outstanding debt owed to Red Bank, must repay it first
    // // the admin must first transfer sufficient amount of secondary asset (UST) to the contract
    // // using `MsgSend` or `Cw20ExecuteMsg::Transfer` prior to migration
    // let debt_amount = config.red_bank.query_user_debt(&deps.querier, &env.contract.address, &config.secondary_asset_info)?;
    // let mut msgs: Vec<CosmosMsg> = vec![];
    // if !debt_amount.is_zero() {
    //     msgs.push(
    //         config.red_bank.repay_msg(&Asset::new(config.secondary_asset_info.clone(), debt_amount))?,
    //     );
    // }

    // // query the current bond amount
    // // if there is zero bonded assets and zero debt, we simply wipe the storage and return
    // let bond_amount = config.astro_generator.query_bonded_amount(&deps.querier, &env.contract.address, &config.primary_pair.liquidity_token)?;
    // if bond_amount.is_zero() {
    //     CONFIG.remove(deps.storage);
    //     STATE.remove(deps.storage);
    //     return Ok(Response::new());
    // }

    // // withdraw locked liquidity from Astro generator
    // let submsg = SubMsg::reply_on_success(
    //     config.astro_generator.unbond_msg(&config.primary_pair.liquidity_token, bond_amount)?,
    //     1,
    // );

    // Ok(Response::new().add_messages(msgs).add_submessage(submsg))

    Ok(Response::new())
}
