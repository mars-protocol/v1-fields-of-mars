use cosmwasm_std::{
    entry_point, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order, Reply,
    Response, StdError, StdResult, SubMsg, SubMsgExecutionResponse,
};
use cw_asset::{Asset, AssetInfo};

use crate::state::{CONFIG, POSITION, STATE};

#[entry_point]
pub fn instantiate(_deps: DepsMut, _env: Env, _info: MessageInfo, _msg: Empty) -> StdResult<Response> {
    Err(StdError::generic_err("`instantiate` is not implemented"))
}

#[entry_point]
pub fn execute(_deps: DepsMut, _env: Env, _info: MessageInfo, _msg: Empty) -> StdResult<Response> {
    Err(StdError::generic_err("`execute` is not implemented"))
}

#[entry_point]
pub fn query(_deps: Deps, _env: Env, _msg: Empty) -> StdResult<Binary> {
    Err(StdError::generic_err("`query` is not implemented"))
}

#[entry_point]
pub fn migrate(deps: DepsMut, env: Env, _msg: Empty) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // can only initiate self-destruction if there is no outstanding debt owed to Red Bank
    let debt_amount = config.red_bank.query_user_debt(&deps.querier, &env.contract.address, &config.secondary_asset_info)?;
    if !debt_amount.is_zero() {
        return Err(StdError::generic_err("must pay off debt before initiating self-destruct"));
    }

    // query the current bond amount
    // if there is zero bonded assets and zero debt, we simply wipe the storage and return
    let bond_amount = config.astro_generator.query_bonded_amount(&deps.querier, &env.contract.address, &config.primary_pair.liquidity_token)?;
    if bond_amount.is_zero() {
        CONFIG.remove(deps.storage);
        STATE.remove(deps.storage);
        return Ok(Response::new());
    }

    // withdraw locked liquidity from Astro generator
    let submsg = SubMsg::reply_on_success(
        config.astro_generator.unbond_msg(&config.primary_pair.liquidity_token, bond_amount)?,
        1,
    );

    Ok(Response::new().add_submessage(submsg))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> StdResult<Response> {
    match reply.id {
        1 => after_unbond(deps, env),
        2 => after_withdraw_liquidity(deps, env),
        _ => Ok(Response::new()),
    }
}

pub fn after_unbond(deps: DepsMut, env: Env) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // query if the contract has ASTRO token; if yes, sell it for UST
    let astro_amount = config.astro_token_info.query_balance(&deps.querier, &env.contract.address)?;
    let mut submsgs: Vec<SubMsg> = vec![];
    if !astro_amount.is_zero() {
        submsgs.push(
            config.astro_pair.swap_submsg(0, &Asset::new(config.astro_token_info.clone(), astro_amount), None, None)?,
        );
    }

    // query how much LP shares we just received
    let shares_amount = AssetInfo::Cw20(config.primary_pair.liquidity_token.clone()).query_balance(&deps.querier, &env.contract.address)?;

    // burn LP shares
    let submsg = config.primary_pair.withdraw_submsg(2, shares_amount)?;

    Ok(Response::new()
        .add_submessages(submsgs)
        .add_submessage(submsg))
}

pub fn after_withdraw_liquidity(deps: DepsMut, env: Env) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // query how much primary and secondary assets are available
    let mut primary_amount = config.primary_asset_info.query_balance(&deps.querier, &env.contract.address)?;
    let mut secondary_amount = config.secondary_asset_info.query_balance(&deps.querier, &env.contract.address)?;

    // find all user positions
    // NOTE: here we collect all user positions into a giant `Vec`. the assumption is that the number
    // of remaining positions is not too big.
    let user_units = POSITION
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (user_bytes, position) = item?;
            let user = String::from_utf8(user_bytes)?;
            Ok((user, position.bond_units))
        })
        .collect::<StdResult<Vec<_>>>()?;

    // refund the users
    let mut msgs: Vec<CosmosMsg> = vec![];
    for (user, bond_units) in user_units {
        let primary_refund_amount = primary_amount.multiply_ratio(bond_units, state.total_bond_units);
        msgs.push(
            Asset::new(config.primary_asset_info.clone(), primary_refund_amount).transfer_msg(&user)?,
        );

        let secondary_refund_amount = secondary_amount.multiply_ratio(bond_units, state.total_bond_units);
        msgs.push(
            Asset::new(config.secondary_asset_info.clone(), secondary_refund_amount).transfer_msg(&user)?,
        );

        primary_amount -= primary_refund_amount;
        secondary_amount -= secondary_refund_amount;
        state.total_bond_units -= bond_units;

        POSITION.remove(deps.storage, &Addr::unchecked(user));
    }

    // amounts and total units should all be zeroes at this point; otherwise something is wrong
    if !primary_amount.is_zero() {
        return Err(StdError::generic_err(
            format!("`primary_amount` is non-zero ({}). wtf?", primary_amount),
        ));
    }
    if !secondary_amount.is_zero() {
        return Err(StdError::generic_err(
            format!("`secondary_amount` is non-zero ({}). wtf?", secondary_amount),
        ));
    }
    if !state.total_bond_units.is_zero() {
        return Err(StdError::generic_err(
            format!("`state.total_bond_units` is non-zero ({}). wtf?", state.total_bond_units),
        ));
    }

    // wipe the storage, remove unnecessary data from the blockchain's state. good practice!
    CONFIG.remove(deps.storage);
    STATE.remove(deps.storage);

    Ok(Response::new().add_messages(msgs))
}

pub fn unwrap_reply(reply: Reply) -> StdResult<SubMsgExecutionResponse> {
    reply.result.into_result().map_err(StdError::generic_err)
}
