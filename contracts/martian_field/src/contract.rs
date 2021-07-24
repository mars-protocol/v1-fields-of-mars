#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    attr, to_binary, Addr, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdError, StdResult, SubMsg, Uint128,
};
use field_of_mars::{
    asset::{Asset, AssetInfo},
    martian_field::{
        CallbackMsg, ConfigResponse, ExecuteMsg, HealthResponse, InstantiateMsg,
        MigrateMsg, PositionResponse, QueryMsg, SnapshotResponse, StateResponse,
    },
};

use crate::state::{Position, Snapshot, State, CONFIG, POSITION, SNAPSHOT, STATE};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    CONFIG.save(deps.storage, &msg)?;
    STATE.save(deps.storage, &State::new())?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::IncreasePosition {
            deposits,
        } => increase_position(deps, env, info, deposits),
        ExecuteMsg::ReducePosition {
            bond_units,
            remove,
            repay,
        } => reduce_position(deps, env, info, bond_units, remove, repay),
        ExecuteMsg::PayDebt {
            user,
            deposit,
        } => pay_debt(deps, env, info, user, deposit),
        ExecuteMsg::Harvest {
            ..
        } => harvest(deps, env, info),
        ExecuteMsg::ClosePosition {
            user,
        } => close_position(deps, env, info, user),
        ExecuteMsg::Liquidate {
            user,
            deposit,
        } => liquidate(deps, env, info, user, deposit),
        ExecuteMsg::UpdateConfig {
            new_config,
        } => update_config(deps, env, info, new_config),
        ExecuteMsg::Callback(msg) => _handle_callback(deps, env, info, msg),
    }
}

fn _handle_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> StdResult<Response> {
    // Callback functions can only be called this contract itself
    if info.sender != env.contract.address {
        return Err(StdError::generic_err("callbacks cannot be invoked externally"));
    }
    match msg {
        CallbackMsg::ProvideLiquidity {
            user,
        } => _provide_liquidity(deps, env, user),
        CallbackMsg::RemoveLiquidity {
            user,
        } => _remove_liquidity(deps, env, user),
        CallbackMsg::Bond {
            user,
        } => _bond(deps, env, user),
        CallbackMsg::Unbond {
            user,
            bond_units,
        } => _unbond(deps, env, user, bond_units),
        CallbackMsg::Borrow {
            user,
            amount,
        } => _borrow(deps, env, user, amount),
        CallbackMsg::Repay {
            user,
        } => _repay(deps, env, user),
        CallbackMsg::Refund {
            user,
            recipient,
            percentage,
        } => _refund(deps, env, user, recipient, percentage),
        CallbackMsg::Reinvest {
            amount,
        } => _reinvest(deps, env, amount),
        CallbackMsg::Snapshot {
            user,
        } => _snapshot(deps, env, user),
        CallbackMsg::AssertHealth {
            user,
        } => _assert_health(deps, env, user),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps, env)?),
        QueryMsg::State {} => to_binary(&query_state(deps, env)?),
        QueryMsg::Position {
            user,
        } => to_binary(&query_position(deps, env, user)?),
        QueryMsg::Health {
            user,
        } => to_binary(&query_health(deps, env, user)?),
        QueryMsg::Snapshot {
            user,
        } => to_binary(&query_snapshot(deps, env, user)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Err(StdError::generic_err("unimplemented"))
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

fn increase_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    deposits: [Asset; 2],
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    let mut position =
        POSITION.load(deps.storage, &info.sender).unwrap_or(Position::new(&config));

    // Find how much long and short assets where received, respectively
    let long_deposited = deposits
        .iter()
        .find(|deposit| &deposit.info == &config.long_asset)
        .unwrap()
        .amount;
    let short_deposited = deposits
        .iter()
        .find(|deposit| &deposit.info == &config.short_asset)
        .unwrap()
        .amount;

    // Query asset depths of the AMM pool
    let pool_info =
        config.swap.query_pool(&deps.querier, &config.long_asset, &config.short_asset)?;

    // Calculate how much short asset is need for liquidity provision
    // Note: We don't check whether `pool_info.long_depth` is zero here because in practice
    // it should always be greater than zero
    let short_needed =
        long_deposited.multiply_ratio(pool_info.short_depth, pool_info.long_depth);

    // Calculate how much short asset to borrow from Mars
    let short_to_borrow =
        short_needed.checked_sub(short_deposited).unwrap_or(Uint128::zero());

    // Increment the user's unlocked asset amounts
    position.unlocked_assets[0].amount += long_deposited;
    position.unlocked_assets[1].amount += short_deposited;
    POSITION.save(deps.storage, &info.sender, &position)?;

    // Prepare messages
    let mut messages: Vec<SubMsg> = vec![];

    // For each deposit,
    // If it's a CW20, we transfer it from the user's wallet (must have allowance)
    // If it's a native token, we assert the amount was transferred with the message
    for deposit in deposits.iter() {
        match &deposit.info {
            AssetInfo::Token {
                ..
            } => {
                messages.push(
                    deposit.transfer_from_msg(&info.sender, &env.contract.address)?,
                );
            }
            AssetInfo::NativeToken {
                ..
            } => {
                deposit.assert_sent_fund(&info)?;
            }
        }
    }

    // Note: callback messages need to be converted to SubMsg type
    let callbacks = [
        CallbackMsg::Borrow {
            user: info.sender.clone(),
            amount: short_to_borrow,
        },
        CallbackMsg::ProvideLiquidity {
            user: info.sender.clone(),
        },
        CallbackMsg::Bond {
            user: info.sender.clone(),
        },
        CallbackMsg::AssertHealth {
            user: info.sender.clone(),
        },
        CallbackMsg::Snapshot {
            user: info.sender.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.to_submsg(&env.contract.address).unwrap()),
    );

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::IncreasePosition"),
            attr("user", info.sender),
            attr("long_deposited", long_deposited),
            attr("short_deposited", short_deposited),
        ],
        events: vec![],
        data: None,
    })
}

fn reduce_position(
    _deps: DepsMut,
    env: Env,
    info: MessageInfo,
    bond_units: Option<Uint128>,
    remove: bool,
    repay: bool,
) -> StdResult<Response> {
    let mut callbacks = vec![CallbackMsg::Unbond {
        user: info.sender.clone(),
        bond_units,
    }];

    if remove {
        callbacks.push(CallbackMsg::RemoveLiquidity {
            user: info.sender.clone(),
        });
    }

    if repay {
        callbacks.push(CallbackMsg::Repay {
            user: info.sender.clone(),
        });
    }

    callbacks.extend(vec![
        CallbackMsg::AssertHealth {
            user: info.sender.clone(),
        },
        CallbackMsg::Refund {
            user: info.sender.clone(),
            recipient: info.sender.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::Snapshot {
            user: info.sender.clone(),
        },
    ]);

    let messages = callbacks
        .iter()
        .map(|msg| msg.to_submsg(&env.contract.address).unwrap())
        .collect();

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::ReducePosition"),
            attr("user", info.sender),
        ],
        events: vec![],
        data: None,
    })
}

fn pay_debt(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: Option<String>,
    deposit: Asset,
) -> StdResult<Response> {
    let user_addr = if let Some(user) = user {
        deps.api.addr_validate(&user)?
    } else {
        info.sender.clone()
    };

    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // Make sure the asset deposited is the short asset
    if deposit.info != config.short_asset {
        return Err(StdError::generic_err(format!(
            "must deposit {}!",
            config.short_asset.query_denom(&deps.querier)?
        )));
    }

    // Increment the user's unlocked short asset amount
    position.unlocked_assets[1].amount += deposit.amount;
    POSITION.save(deps.storage, &user_addr, &position)?;

    let mut messages: Vec<SubMsg> = vec![];

    // Receive the deposit
    match &deposit.info {
        AssetInfo::Token {
            ..
        } => {
            messages.push(deposit.transfer_from_msg(&user_addr, &env.contract.address)?);
        }
        AssetInfo::NativeToken {
            ..
        } => {
            deposit.assert_sent_fund(&info)?;
        }
    }

    let callbacks = [
        CallbackMsg::Repay {
            user: user_addr.clone(),
        },
        CallbackMsg::Snapshot {
            user: user_addr.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.to_submsg(&env.contract.address).unwrap()),
    );

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::PayDebt"),
            attr("payer", info.sender),
            attr("user", String::from(user_addr)),
            attr("short_deposited", deposit.amount),
        ],
        events: vec![],
        data: None,
    })
}

fn harvest(deps: DepsMut, env: Env, info: MessageInfo) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // Only keepers can harvest
    if config.keepers.iter().all(|keeper| keeper != &info.sender) {
        return Err(StdError::generic_err("only whitelisted keepers can harvest!"));
    }

    // Query the amount of reward to expect to receive
    let reward_amount =
        config.staking.query_reward(&deps.querier, &env.contract.address)?;

    let mut messages = vec![config.staking.withdraw_msg()?];

    let callbacks = [
        CallbackMsg::Reinvest {
            amount: reward_amount,
        },
        CallbackMsg::ProvideLiquidity {
            user: env.contract.address.clone(),
        },
        CallbackMsg::Bond {
            user: env.contract.address.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.to_submsg(&env.contract.address).unwrap()),
    );

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::Harvest"),
            attr("reward_amount", reward_amount),
        ],
        events: vec![],
        data: None,
    })
}

fn close_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
) -> StdResult<Response> {
    let user_addr = deps.api.addr_validate(&user)?;
    let config = CONFIG.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr)?;

    // Only active positions can be closed
    if !position.is_active() {
        return Err(StdError::generic_err("position is already closed!"));
    }

    let health_info = query_health(deps.as_ref(), env.clone(), Some(user.clone()))?;
    let ltv = health_info.ltv.unwrap();

    // Only positions with unhealthy LTV's can be closed
    // Since the position is active, we can safely unwrap it here
    if ltv <= config.max_ltv {
        return Err(StdError::generic_err(format!("position is healthy! ltv: {}", ltv)));
    }

    let callbacks = [
        CallbackMsg::Unbond {
            user: user_addr.clone(),
            bond_units: None,
        },
        CallbackMsg::RemoveLiquidity {
            user: user_addr.clone(),
        },
        CallbackMsg::Repay {
            user: user_addr.clone(),
        },
    ];

    let messages = callbacks
        .iter()
        .map(|msg| msg.to_submsg(&env.contract.address).unwrap())
        .collect();

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::ClosePosition"),
            attr("user", user),
            attr("ltv", ltv),
            attr("liquidator", info.sender),
        ],
        events: vec![],
        data: None,
    })
}

fn liquidate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user: String,
    deposit: Asset,
) -> StdResult<Response> {
    let user_addr = deps.api.addr_validate(&user)?;
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user_addr)?;

    // The position must have been closed
    if position.is_active() {
        return Err(StdError::generic_err("cannot liquidate an active position!"));
    }

    // Make sure the asset deposited is the short asset
    if deposit.info != config.short_asset {
        return Err(StdError::generic_err("must deposit short asset!"));
    }

    // Calculate percentage of unlocked asset that should be accredited to the liquidator
    let total_debt = config.red_bank.query_debt(
        &deps.querier,
        &env.contract.address,
        &config.short_asset,
    )?;

    let debt_amount =
        total_debt.multiply_ratio(position.debt_units, state.total_debt_units);

    let deliverable_amount =
        config.short_asset.deduct_tax(&deps.querier, deposit.amount)?;

    let percentage = if deliverable_amount > debt_amount {
        Decimal::one()
    } else {
        Decimal::from_ratio(deliverable_amount, debt_amount)
    };

    // Increment the user's unlocked short asset amount
    position.unlocked_assets[1].amount += deposit.amount;
    POSITION.save(deps.storage, &user_addr, &position)?;

    let mut messages: Vec<SubMsg> = vec![];

    // Receive the deposit
    match &deposit.info {
        AssetInfo::Token {
            ..
        } => {
            messages.push(deposit.transfer_from_msg(&user_addr, &env.contract.address)?);
        }
        AssetInfo::NativeToken {
            ..
        } => {
            deposit.assert_sent_fund(&info)?;
        }
    }

    let callbacks = [
        CallbackMsg::Repay {
            user: user_addr.clone(),
        },
        CallbackMsg::Refund {
            user: user_addr.clone(),
            recipient: info.sender.clone(),
            percentage,
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.to_submsg(&env.contract.address).unwrap()),
    );

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::ExecuteMsg::Liquidate"),
            attr("user", user),
            attr("liquidator", info.sender),
            attr("short_deposited", deposit.amount),
        ],
        events: vec![],
        data: None,
    })
}

fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: InstantiateMsg,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.governance {
        return Err(StdError::generic_err("only governance can update config!"));
    }

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![attr("action", "martian_field::ExecuteMsg::UpdateConfig")],
        events: vec![],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

fn _provide_liquidity(deps: DepsMut, _env: Env, user: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    // Assets to be provided to the AMM
    // Note: must deduct tax!
    let deposits = [
        position.unlocked_assets[0].deduct_tax(&deps.querier)?, // long asset
        position.unlocked_assets[1].deduct_tax(&deps.querier)?, // short asset
    ];

    // The total costs for deposits, including tax
    let costs = [
        deposits[0].add_tax(&deps.querier)?, // long asset
        deposits[1].add_tax(&deps.querier)?, // short asset
    ];

    // The amount of shares to expect to receive
    let shares = config.swap.simulate_provide(&deps.querier, &deposits)?;

    // Update unlocked asset amounts
    position.unlocked_assets[0].amount -= costs[0].amount; // long asset
    position.unlocked_assets[1].amount -= costs[1].amount; // short asset
    position.unlocked_assets[2].amount += shares; // share tokens
    POSITION.save(deps.storage, &user, &position)?;

    Ok(Response {
        messages: config.swap.provide_msgs(&deposits)?,
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::ProvideLiquidity"),
            attr("user", user),
            attr("long_provided", deposits[0].amount),
            attr("short_provided", deposits[1].amount),
            attr("shares_received", shares),
        ],
        events: vec![],
        data: None,
    })
}

fn _remove_liquidity(deps: DepsMut, _env: Env, user: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    // Amount of shares to burn
    let shares = position.unlocked_assets[2].amount;

    // Calculate the return amount of assets
    // Note: must deduct tax! (`simulate_remove` function does this)
    let return_amounts = config.swap.simulate_remove(
        &deps.querier,
        shares,
        &config.long_asset,
        &config.short_asset,
    )?;

    // Update unlocked asset amounts
    position.unlocked_assets[0].amount += return_amounts[0];
    position.unlocked_assets[1].amount += return_amounts[1];
    position.unlocked_assets[2].amount -= shares;
    POSITION.save(deps.storage, &user, &position)?;

    Ok(Response {
        messages: vec![config.swap.withdraw_msg(shares)?],
        attributes: vec![
            attr("action", "field_of_mars::CallbackMsg::RemoveLiquidity"),
            attr("shares_burned", shares),
            attr("long_received", return_amounts[0]),
            attr("short_received", return_amounts[1]),
        ],
        events: vec![],
        data: None,
    })
}

fn _bond(deps: DepsMut, env: Env, user: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    // Amount of shares to bond
    let bond_amount = position.unlocked_assets[2].amount;

    // Total amount of bonded shares the contract currently has
    let total_bond = config.staking.query_bond(&deps.querier, &env.contract.address)?;

    // Calculate how many bond units the user should be accredited
    // We define the initial bond unit = 100,000 units per share bonded
    let mut bond_units_to_add = if total_bond.is_zero() {
        bond_amount.multiply_ratio(1_000_000u128, 1u128)
    } else {
        state.total_bond_units.multiply_ratio(bond_amount, total_bond)
    };

    // If user is the contract itself, which is the case during harvest, then we don't
    // increment bond units
    if user == env.contract.address {
        bond_units_to_add = Uint128::zero();
    }

    // Update state
    state.total_bond_units += bond_units_to_add;
    STATE.save(deps.storage, &state)?;

    // Update position
    position.bond_units += bond_units_to_add;
    position.unlocked_assets[2].amount -= bond_amount;
    POSITION.save(deps.storage, &user, &position)?;

    Ok(Response {
        messages: vec![config.staking.bond_msg(bond_amount)?],
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::Bond"),
            attr("user", user),
            attr("bond_amount", bond_amount),
            attr("bond_units_added", bond_units_to_add),
        ],
        events: vec![],
        data: None,
    })
}

fn _unbond(
    deps: DepsMut,
    env: Env,
    user: Addr,
    bond_units: Option<Uint128>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    // Unbond all if `bond_units` is not provided
    let bond_units_to_reduce = bond_units.unwrap_or(position.bond_units);

    // Total amount of share tokens bonded in the staking contract
    let total_bond = config.staking.query_bond(&deps.querier, &env.contract.address)?;

    // Amount of shares to unbond
    let unbond_amount =
        total_bond.multiply_ratio(bond_units_to_reduce, state.total_bond_units);

    // Update state
    state.total_bond_units -= bond_units_to_reduce;
    STATE.save(deps.storage, &state)?;

    // Update position
    position.bond_units -= bond_units_to_reduce;
    position.unlocked_assets[2].amount += unbond_amount;
    POSITION.save(deps.storage, &user, &position)?;

    Ok(Response {
        messages: vec![config.staking.unbond_msg(unbond_amount)?],
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::Unbond"),
            attr("user", user),
            attr("unbond_amount", unbond_amount),
            attr("bond_units_reduced", bond_units_to_reduce),
        ],
        events: vec![],
        data: None,
    })
}

fn _borrow(deps: DepsMut, env: Env, user: Addr, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    let response = if !amount.is_zero() {
        // Total amount of short asset owed by the contract to Mars
        let total_debt = config.red_bank.query_debt(
            &deps.querier,
            &env.contract.address,
            &config.short_asset,
        )?;

        // Calculate how many debt units the user should be accredited
        // We define the initial debt unit = 100,000 units per short asset borrowed
        let debt_units_to_add = if total_debt.is_zero() {
            amount.multiply_ratio(1_000_000u128, 1u128)
        } else {
            state.total_debt_units.multiply_ratio(amount, total_debt)
        };

        // The receivable amount after tax
        let amount_after_tax = config.short_asset.deduct_tax(&deps.querier, amount)?;

        // Update storage
        state.total_debt_units += debt_units_to_add;
        STATE.save(deps.storage, &state)?;

        // Update position
        position.debt_units += debt_units_to_add;
        position.unlocked_assets[1].amount += amount_after_tax;
        POSITION.save(deps.storage, &user, &position)?;

        // Generate message
        let borrow_message = config.red_bank.borrow_msg(&Asset {
            info: config.short_asset.clone(),
            amount,
        })?;

        Response {
            messages: vec![borrow_message],
            attributes: vec![
                attr("action", "martial_field::CallbackMsg::Borrow"),
                attr("user", user),
                attr("amount", amount),
                attr("amount_after_tax", amount_after_tax),
                attr("debt_units_added", debt_units_to_add),
            ],
            events: vec![],
            data: None,
        }
    } else {
        Response {
            messages: vec![],
            attributes: vec![
                attr("action", "martian_field::CallbackMsg::Borrow"),
                attr("warning", "skipped: borrow amount is zero!"),
            ],
            events: vec![],
            data: None,
        }
    };

    Ok(response)
}

fn _repay(deps: DepsMut, env: Env, user: Addr) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &user)?;

    // Amount of short asset to repay
    let unlocked_short = position.unlocked_assets[1].amount;

    // If there is asset available for repayment AND there is debt to be paid
    let response = if !unlocked_short.is_zero() && !position.debt_units.is_zero() {
        // Total amount of short asset owed by the contract to Mars
        let total_debt = config.red_bank.query_debt(
            &deps.querier,
            &env.contract.address,
            &config.short_asset,
        )?;

        // Amount of debt assigned to the user
        // Nost: We already have `position.debt_units` != 0 so `state.total_debt_units` is
        // necessarily non-zero. No need to check for division by zero here
        let debt_amount =
            total_debt.multiply_ratio(position.debt_units, state.total_debt_units);

        // Due to tax, the amount of `repay_asset` received may not be fully delivered to
        // Mars. Calculate the maximum deliverable amount.
        let deliverable_amount =
            config.short_asset.deduct_tax(&deps.querier, unlocked_short)?;

        // The amount to repay is the deliverable amount of the user's unlocked asset, or
        // the user's outstanding debt, whichever is smaller
        let repay_amount = std::cmp::min(deliverable_amount, debt_amount);

        // Total amount of short asset that will be deducted from the user's balance
        let repay_cost = config.short_asset.add_tax(&deps.querier, repay_amount)?;

        // The amount of debt units to reduce
        // Note: Same, `debt_amount` is necessarily non-zero
        let debt_units_to_reduce =
            position.debt_units.multiply_ratio(repay_amount, debt_amount);

        // Update state
        state.total_debt_units -= debt_units_to_reduce;
        STATE.save(deps.storage, &state)?;

        // Update position
        position.debt_units -= debt_units_to_reduce;
        position.unlocked_assets[1].amount -= repay_cost;
        POSITION.save(deps.storage, &user, &position)?;

        // Generate message
        let repay_message = config.red_bank.repay_msg(&Asset {
            info: config.short_asset.clone(),
            amount: repay_amount,
        })?;

        Response {
            messages: vec![repay_message],
            attributes: vec![
                attr("action", "martian_field::CallbackMsg::Repay"),
                attr("user", user),
                attr("repay_amount", repay_amount),
                attr("repay_cost", repay_cost),
                attr("debt_units_reduced", debt_units_to_reduce),
            ],
            events: vec![],
            data: None,
        }
    } else {
        Response {
            messages: vec![],
            attributes: vec![
                attr("action", "martian_field::CallbackMsg::Repay"),
                attr("warning", "skipped: repay amount is zero!"),
            ],
            events: vec![],
            data: None,
        }
    };

    Ok(response)
}

fn _refund(
    deps: DepsMut,
    _env: Env,
    user: Addr,
    recipient: Addr,
    percentage: Decimal,
) -> StdResult<Response> {
    let mut position = POSITION.load(deps.storage, &user)?;

    // Apply percentage
    let assets: Vec<Asset> = position
        .unlocked_assets
        .to_vec()
        .iter()
        .map(|asset| Asset {
            info: asset.info.clone(),
            amount: asset
                .info
                .deduct_tax(&deps.querier, asset.amount * percentage)
                .unwrap(),
        })
        .collect();

    // The transfer cost for refunding
    let costs: Vec<Asset> =
        assets.iter().map(|asset| asset.add_tax(&deps.querier).unwrap()).collect();

    // Update position
    position.unlocked_assets[0].amount -= costs[0].amount;
    position.unlocked_assets[1].amount -= costs[1].amount;
    position.unlocked_assets[2].amount -= costs[2].amount;
    POSITION.save(deps.storage, &user, &position)?;

    // Generate messages for the transfers
    // Notes:
    // 1. Must filter off assets whose amounts are zero
    // 2. `asset.transfer_message` does tax deduction so no need to do it here
    let messages = assets
        .iter()
        .filter(|asset| !asset.amount.is_zero())
        .map(|asset| asset.transfer_msg(&recipient).unwrap())
        .collect();

    Ok(Response {
        messages,
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::Refund"),
            attr("user", user),
            attr("recipient", recipient),
            attr("percentage", percentage),
            attr("long_refunded", assets[0].amount),
            attr("long_remaining", position.unlocked_assets[0].amount),
            attr("short_refunded", assets[1].amount),
            attr("short_remaining", position.unlocked_assets[1].amount),
            attr("shares_refunded", assets[2].amount),
            attr("shares_remaining", position.unlocked_assets[2].amount),
        ],
        events: vec![],
        data: None,
    })
}

fn _reinvest(deps: DepsMut, env: Env, amount: Uint128) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let treasury = deps.api.addr_validate(&config.treasury)?;

    let mut position = POSITION
        .load(deps.storage, &env.contract.address)
        .unwrap_or(Position::new(&config));

    // Calculate how much performance fee should be charged
    let fee = amount * config.fee_rate;
    let amount_after_fee = amount - fee;

    // Half of the reward is to be retained, not swapped
    let retain_amount = amount_after_fee.multiply_ratio(1u128, 2u128);

    // The amount of reward to be swapped
    // Note: here we assume `long_token` == `reward_token`. This is the case for popular
    // farms e.g. ANC, MIR, MINE, but not for mAsset farms.
    // MAsset support may be added in a future version
    let offer_amount = amount_after_fee - retain_amount;
    let offer_after_tax = config.long_asset.deduct_tax(&deps.querier, offer_amount)?;

    // Note: must deduct tax here
    let offer_asset = Asset {
        info: config.long_asset.clone(),
        amount: offer_after_tax,
    };

    // Calculate the return amount of the swap
    // Note: must deduct_tax here
    let return_amount = config.swap.simulate_swap(&deps.querier, &offer_asset)?;
    let return_after_tax = config.short_asset.deduct_tax(&deps.querier, return_amount)?;

    // Update position
    position.unlocked_assets[0].amount += retain_amount;
    position.unlocked_assets[1].amount += return_after_tax;
    POSITION.save(deps.storage, &env.contract.address, &position)?;

    Ok(Response {
        messages: vec![
            config.long_asset.transfer_msg(&treasury, fee)?,
            config.swap.swap_msg(&offer_asset)?,
        ],
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::Reinvest"),
            attr("amount", amount),
            attr("fee_amount", fee),
            attr("retain_amount", retain_amount),
            attr("offer_amount", offer_amount),
            attr("offer_after_tax", offer_after_tax),
            attr("return_amount", return_amount),
            attr("return_after_tax", return_after_tax),
        ],
        events: vec![],
        data: None,
    })
}

fn _snapshot(deps: DepsMut, env: Env, user: Addr) -> StdResult<Response> {
    let position = POSITION.load(deps.storage, &user)?;

    let snapshot = Snapshot {
        time: env.block.time,
        height: env.block.height,
        health: query_health(deps.as_ref(), env, Some(String::from(&user)))?,
        position,
    };

    SNAPSHOT.save(deps.storage, &user, &snapshot)?;

    Ok(Response {
        messages: vec![],
        attributes: vec![
            attr("action", "martian_field::CallbackMsg::Snapshot"),
            attr("user", user),
        ],
        events: vec![],
        data: None,
    })
}

fn _assert_health(deps: DepsMut, env: Env, user: Addr) -> StdResult<Response> {
    let config_raw = CONFIG.load(deps.storage)?;
    let health_info = query_health(deps.as_ref(), env, Some(String::from(&user)))?;

    // If ltv is Some(ltv), we assert it is no larger than `config.max_ltv`
    // If it is None, meaning `bond_value` is zero, we assert debt is also zero
    let healthy = match health_info.ltv {
        Some(ltv) => {
            if ltv <= config_raw.max_ltv {
                true
            } else {
                false
            }
        }
        None => {
            if health_info.debt_value.is_zero() {
                true
            } else {
                false
            }
        }
    };

    // Convert `ltv` to String so that it can be recorded in logs
    let ltv_str = if let Some(ltv) = health_info.ltv {
        format!("{}", ltv)
    } else {
        String::from("null")
    };

    if healthy {
        Ok(Response {
            messages: vec![],
            attributes: vec![
                attr("action", "martian_field::CallbackMsg::AssertHealth"),
                attr("user", user),
                attr("ltv", ltv_str),
            ],
            events: vec![],
            data: None,
        })
    } else {
        Err(StdError::generic_err(format!("ltv is greater than threshold: {}", ltv_str)))
    }
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

fn query_config(deps: Deps, _env: Env) -> StdResult<ConfigResponse> {
    CONFIG.load(deps.storage)
}

fn query_state(deps: Deps, _env: Env) -> StdResult<StateResponse> {
    STATE.load(deps.storage)
}

/// @dev Panics if the user hasn't had a position (error 500)
fn query_position(deps: Deps, _env: Env, user: String) -> StdResult<PositionResponse> {
    POSITION.load(deps.storage, &deps.api.addr_validate(&user)?)
}

/// @dev Panics if the user hasn't had a position (error 500)
fn query_snapshot(deps: Deps, _env: Env, user: String) -> StdResult<SnapshotResponse> {
    SNAPSHOT.load(deps.storage, &deps.api.addr_validate(&user)?)
}

/// @dev Returns `None` (serialized into `null`) if `bond_units = 0` which is the case for
/// closed positions.
/// @dev Panics if the user hasn't had a position (error 500)
fn query_health(deps: Deps, env: Env, user: Option<String>) -> StdResult<HealthResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // If user address is provided, we read bond/debt units from storage
    // Otherwise, use total units, in which case calculates the strategy's overall health
    let (bond_units, debt_units) = if let Some(user) = user {
        let position = POSITION.load(deps.storage, &deps.api.addr_validate(&user)?)?;
        (position.bond_units, position.debt_units)
    } else {
        (state.total_bond_units, state.total_debt_units)
    };

    // Part 1. Query of necessary info
    // Total amount of share tokens bonded in the staking contract
    let total_bond = config.staking.query_bond(&deps.querier, &env.contract.address)?;

    // Total amount of debt owed to Mars
    let total_debt = config.red_bank.query_debt(
        &deps.querier,
        &env.contract.address,
        &config.short_asset,
    )?;

    // Info of the AMM pool
    let pool_info =
        config.swap.query_pool(&deps.querier, &config.long_asset, &config.short_asset)?;

    // Part 2. Calculating value of the user's bonded assets
    // valueOfThePool = longDepth * valueOfLongAsset + shortDepth = 2 * shortDepth
    // totalBondValue = valueOfPool * strategyShares / totalShares
    // bondValue = totalBondValue * bondUnits / totalBondUnits
    // Note:
    // 1. bond value is measured in the short asset
    // 2. must handle the case where `total_bond_units` = 0
    let bond_value = if state.total_bond_units.is_zero() {
        Uint128::zero()
    } else {
        (pool_info.short_depth + pool_info.short_depth)
            .multiply_ratio(total_bond, pool_info.share_supply)
            .multiply_ratio(bond_units, state.total_bond_units)
    };

    // Part 2. Calculating value of the user's debt
    // Note: must handle the case where `total_debt_units = 0`
    let debt_value = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_debt.multiply_ratio(debt_units, state.total_debt_units)
    };

    // Part 3. Calculating LTV
    // Note: must handle division by zero!
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
