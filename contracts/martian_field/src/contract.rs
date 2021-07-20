use cosmwasm_std::{
    log, to_binary, Api, Binary, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, MigrateResponse, Querier, StdError, StdResult, Storage,
    Uint128,
};

use field_of_mars::{
    asset::{Asset, AssetInfo},
    martian_field::{
        CallbackMsg, ConfigResponse, HandleMsg, HealthResponse, InitMsg, MigrateMsg,
        PositionResponse, QueryMsg, SnapshotResponse, StateResponse,
    },
};

use crate::state::{Config, Position, Snapshot, State};

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let contract_addr = deps.api.canonical_address(&env.contract.address)?;
    Config::from_init_msg(&deps, &msg)?.write(&mut deps.storage)?;
    State::new(&contract_addr).write(&mut deps.storage)?;
    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::IncreasePosition {
            deposits,
        } => increase_position(deps, env, deposits),
        HandleMsg::ReducePosition {
            bond_units,
            remove,
            repay,
        } => reduce_position(deps, env, bond_units, remove, repay),
        HandleMsg::ClosePosition {
            user,
        } => close_position(deps, env, user),
        HandleMsg::PayDebt {
            user,
            deposit,
        } => pay_debt(deps, env, user, deposit),
        HandleMsg::Liquidate {
            user,
            deposit,
        } => liquidate(deps, env, user, deposit),
        HandleMsg::Harvest {
            ..
        } => harvest(deps, env),
        HandleMsg::UpdateConfig {
            new_config,
        } => update_config(deps, env, new_config),
        HandleMsg::Callback(msg) => _handle_callback(deps, env, msg),
    }
}

fn _handle_callback<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: CallbackMsg,
) -> StdResult<HandleResponse> {
    // Callback functions can only be called this contract itself
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
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
        CallbackMsg::Swap {
            amount,
        } => _swap(deps, env, amount),
        CallbackMsg::Refund {
            user,
            recipient,
            percentage,
        } => _refund(deps, env, user, recipient, percentage),
        CallbackMsg::Snapshot {
            user,
        } => _snapshot(deps, env, user),
        CallbackMsg::Purge {
            user,
        } => _purge(deps, env, user),
        CallbackMsg::AssertHealth {
            user,
        } => _assert_health(deps, env, user),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::State {} => to_binary(&query_state(deps)?),
        QueryMsg::Position {
            user,
        } => to_binary(&query_position(deps, user)?),
        QueryMsg::Health {
            user,
        } => to_binary(&query_health(deps, user)?),
        QueryMsg::Snapshot {
            user,
        } => to_binary(&query_snapshot(deps, user)?),
    }
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<MigrateResponse> {
    Err(StdError::generic_err("unimplemented"))
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

fn increase_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    deposits: [Asset; 2],
) -> StdResult<HandleResponse> {
    let user = env.message.sender.clone();
    let user_raw = deps.api.canonical_address(&user)?;

    let config = Config::read_normal(deps)?;
    let mut position = Position::read_or_new(&deps.storage, &user_raw)?;

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
        config.swap.query_pool(deps, &config.long_asset, &config.short_asset)?;

    // Calculate how much short asset is need for liquidity provision
    // Note: We don't check whether `pool_info.long_depth` is zero here because in practice
    // it should always be greater than zero
    let short_needed =
        long_deposited.multiply_ratio(pool_info.short_depth, pool_info.long_depth);

    // Calculate how much short asset to borrow from Mars
    let short_to_borrow = (short_needed - short_deposited).unwrap_or(Uint128::zero());

    // Increment the user's unlocked asset amounts
    position.unlocked_assets[0].amount += long_deposited;
    position.unlocked_assets[1].amount += short_deposited;
    position.write(&mut deps.storage, &user_raw)?;

    // Prepare messages
    let mut messages: Vec<CosmosMsg> = vec![];

    // For each deposit,
    // If it's a CW20, we transfer it from the user's wallet (must have allowance)
    // If it's a native token, we assert the amount was transferred with the message
    for deposit in deposits.iter() {
        match &deposit.info {
            AssetInfo::Token {
                ..
            } => {
                messages
                    .push(deposit.transfer_from_message(&user, &env.contract.address)?);
            }
            AssetInfo::NativeToken {
                ..
            } => {
                deposit.assert_sent_fund(&env.message);
            }
        }
    }

    // Note: callback messages need to be converted to CosmosMsg type
    let callbacks = [
        CallbackMsg::Borrow {
            user: user.clone(),
            amount: short_to_borrow,
        },
        CallbackMsg::ProvideLiquidity {
            user: user.clone(),
        },
        CallbackMsg::Bond {
            user: user.clone(),
        },
        CallbackMsg::AssertHealth {
            user: user.clone(),
        },
        CallbackMsg::Snapshot {
            user: user.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap()),
    );

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::IncreasePosition"),
            log("user", user),
            log("long_deposited", long_deposited),
            log("short_deposited", short_deposited),
        ],
        data: None,
    })
}

fn reduce_position<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    env: Env,
    bond_units: Option<Uint128>,
    remove: bool,
    repay: bool,
) -> StdResult<HandleResponse> {
    let user = env.message.sender.clone();

    let mut callbacks = vec![CallbackMsg::Unbond {
        user: user.clone(),
        bond_units,
    }];

    if remove {
        callbacks.push(CallbackMsg::RemoveLiquidity {
            user: user.clone(),
        });
    }

    if repay {
        callbacks.push(CallbackMsg::Repay {
            user: user.clone(),
        });
    }

    callbacks.extend(vec![
        CallbackMsg::AssertHealth {
            user: user.clone(),
        },
        CallbackMsg::Refund {
            user: user.clone(),
            recipient: user.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::Snapshot {
            user: user.clone(),
        },
        CallbackMsg::Purge {
            user: user.clone(),
        },
    ]);

    let messages = callbacks
        .iter()
        .map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::ReducePosition"),
            log("user", user),
        ],
        data: None,
    })
}

fn close_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config_raw = Config::read(&deps.storage)?;
    let position = Position::read(&deps.storage, &user_raw)?;

    let health_info = query_health(deps, Some(user.clone()))?;
    let ltv = health_info.ltv.unwrap();

    // The position must be open
    if !position.is_active() {
        return Err(StdError::generic_err("position is already closed"));
    }

    // The position must have an LTV greater than the liquidation threshold
    if ltv <= config_raw.max_ltv {
        return Err(StdError::generic_err("cannot close a healthy position"));
    }

    let callbacks = [
        CallbackMsg::Unbond {
            user: user.clone(),
            bond_units: None,
        },
        CallbackMsg::RemoveLiquidity {
            user: user.clone(),
        },
        CallbackMsg::Repay {
            user: user.clone(),
        },
    ];

    let messages = callbacks
        .iter()
        .map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::ClosePosition"),
            log("user", user),
            log("ltv", ltv),
            log("liquidator", env.message.sender),
        ],
        data: None,
    })
}

fn pay_debt<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: Option<HumanAddr>,
    deposit: Asset,
) -> StdResult<HandleResponse> {
    let user = user.unwrap_or(env.message.sender.clone());
    let user_raw = deps.api.canonical_address(&user)?;

    let config = Config::read_normal(deps)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Make sure the asset deposited is the short asset
    if deposit.info != config.short_asset {
        return Err(StdError::generic_err("invalid deposit"));
    }

    // Increment the user's unlocked short asset amount
    position.unlocked_assets[1].amount += deposit.amount;
    position.write(&mut deps.storage, &user_raw)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    // Receive the deposit
    match &deposit.info {
        AssetInfo::Token {
            ..
        } => {
            messages.push(deposit.transfer_from_message(&user, &env.contract.address)?);
        }
        AssetInfo::NativeToken {
            ..
        } => {
            deposit.assert_sent_fund(&env.message);
        }
    }

    let callbacks = [
        CallbackMsg::Repay {
            user: user.clone(),
        },
        CallbackMsg::Snapshot {
            user: user.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap()),
    );

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::PayDebt"),
            log("user", user),
            log("short_deposited", deposit.amount),
        ],
        data: None,
    })
}

fn harvest<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let config = Config::read_normal(deps)?;

    // Only keepers can harvest
    if config.keepers.iter().all(|keeper| keeper != &env.message.sender) {
        return Err(StdError::unauthorized());
    }

    // Query the amount of reward to expect to receive
    let reward_amount = config.staking.query_reward(deps, &env.contract.address)?;

    let mut messages = vec![config.staking.withdraw_message()?];

    let callbacks = [
        CallbackMsg::Swap {
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
        callbacks.iter().map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap()),
    );

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::Harvest"),
            log("reward_amount", reward_amount),
        ],
        data: None,
    })
}

fn liquidate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    deposit: Asset,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let state = State::read(&deps.storage)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // The position must have been closed
    if position.is_active() {
        return Err(StdError::generic_err("cannot liquidate an active position"));
    }

    // Make sure the asset deposited is the short asset
    if deposit.info != config.short_asset {
        return Err(StdError::generic_err("invalid deposit"));
    }

    // Increment the user's unlocked short asset amount
    position.unlocked_assets[1].amount += deposit.amount;
    position.write(&mut deps.storage, &user_raw)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    // Receive the deposit
    match &deposit.info {
        AssetInfo::Token {
            ..
        } => {
            messages.push(deposit.transfer_from_message(&user, &env.contract.address)?);
        }
        AssetInfo::NativeToken {
            ..
        } => {
            deposit.assert_sent_fund(&env.message);
        }
    }

    // Calculate percentage of unlocked asset that should be accredited to the liquidator
    let total_debt =
        config.red_bank.query_debt(deps, &env.contract.address, &config.short_asset)?;
    let debt_amount =
        total_debt.multiply_ratio(position.debt_units, state.total_debt_units);

    let repay_amount_after_tax = config.short_asset.deduct_tax(deps, deposit.amount)?;
    let percentage = Decimal::from_ratio(repay_amount_after_tax, debt_amount);

    let callbacks = [
        CallbackMsg::Repay {
            user: user.clone(),
        },
        CallbackMsg::Refund {
            user: user.clone(),
            recipient: env.message.sender.clone(),
            percentage,
        },
        CallbackMsg::Purge {
            user: user.clone(),
        },
    ];

    messages.extend(
        callbacks.iter().map(|msg| msg.into_cosmos_msg(&env.contract.address).unwrap()),
    );

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::HandleMsg::Liquidate"),
            log("short_deposited", deposit.amount),
        ],
        data: None,
    })
}

fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    new_config: InitMsg,
) -> StdResult<HandleResponse> {
    let config = Config::read(&deps.storage)?;
    let governance = deps.api.human_address(&config.governance)?;

    if env.message.sender == governance {
        Config::from_init_msg(&deps, &new_config)?.write(&mut deps.storage)?;
        Ok(HandleResponse::default())
    } else {
        Err(StdError::unauthorized())
    }
}

//----------------------------------------------------------------------------------------
// Callback Functions
//----------------------------------------------------------------------------------------

fn _provide_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Assets to be provided to the AMM
    // Note: must deduct tax!
    let deposits = [
        position.unlocked_assets[0].to_normal(deps)?.deduct_tax(deps)?, // long asset
        position.unlocked_assets[1].to_normal(deps)?.deduct_tax(deps)?, // short asset
    ];

    // The amount of shares to expect to receive
    let shares = config.swap.simulate_provide(deps, &deposits)?;

    // Update unlocked asset amounts
    position.unlocked_assets[0].amount = Uint128::zero(); // long asset
    position.unlocked_assets[1].amount = Uint128::zero(); // short asset
    position.unlocked_assets[2].amount += shares; // share tokens
    position.write(&mut deps.storage, &user_raw)?;

    Ok(HandleResponse {
        messages: config.swap.provide_messages(&deposits)?,
        log: vec![
            log("action", "martian_field::CallbackMsg::ProvideLiquidity"),
            log("user", user),
            log("long_provided", deposits[0].amount),
            log("short_provided", deposits[1].amount),
            log("shares_received", shares),
        ],
        data: None,
    })
}

fn _remove_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Amount of shares to burn
    let shares = position.unlocked_assets[2].amount;

    // Calculate the return amount of assets
    // Note: must deduct tax! (`simulate_remove` function does this)
    let return_amounts = config.swap.simulate_remove(
        deps,
        shares,
        &config.long_asset,
        &config.short_asset,
    )?;

    // Update unlocked asset amounts
    position.unlocked_assets[0].amount += return_amounts[0];
    position.unlocked_assets[1].amount += return_amounts[1];
    position.unlocked_assets[2].amount = Uint128::zero();
    position.write(&mut deps.storage, &user_raw)?;

    Ok(HandleResponse {
        messages: vec![config.swap.withdraw_message(shares)?],
        log: vec![
            log("action", "field_of_mars::CallbackMsg::RemoveLiquidity"),
            log("shares_burned", shares),
            log("long_received", return_amounts[0]),
            log("short_received", return_amounts[1]),
        ],
        data: None,
    })
}

fn _bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut state = State::read(&deps.storage)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Amount of shares to bond
    let bond_amount = position.unlocked_assets[2].amount;

    // Total amount of bonded shares the contract currently has
    let total_bond = config.staking.query_bond(deps, &env.contract.address)?;

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
    state.write(&mut deps.storage)?;

    // Update position
    position.bond_units += bond_units_to_add;
    position.unlocked_assets[2].amount = Uint128::zero();
    position.write(&mut deps.storage, &user_raw)?;

    Ok(HandleResponse {
        messages: vec![config.staking.bond_message(bond_amount)?],
        log: vec![
            log("action", "martian_field::CallbackMsg::Bond"),
            log("user", user),
            log("bond_amount", bond_amount),
            log("bond_units_added", bond_units_to_add),
        ],
        data: None,
    })
}

fn _unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    bond_units: Option<Uint128>,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut state = State::read(&deps.storage)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Unbond all if `bond_units` is not provided
    let bond_units_to_reduce = bond_units.unwrap_or(position.bond_units);

    // Calculate how amount of stakine token to unbond
    let total_bond = config.staking.query_bond(deps, &env.contract.address)?;
    let unbond_amount =
        total_bond.multiply_ratio(bond_units_to_reduce, state.total_bond_units);

    // Update state
    state.total_bond_units = (state.total_bond_units - bond_units_to_reduce)?;
    state.write(&mut deps.storage)?;

    // Update position
    position.bond_units = (position.bond_units - bond_units_to_reduce)?;
    position.unlocked_assets[2].amount += unbond_amount;
    position.write(&mut deps.storage, &user_raw)?;

    Ok(HandleResponse {
        messages: vec![config.staking.unbond_message(unbond_amount)?],
        log: vec![
            log("action", "martian_field::CallbackMsg::Unbond"),
            log("user", user),
            log("unbond_amount", unbond_amount),
            log("bond_units_reduced", bond_units_to_reduce),
        ],
        data: None,
    })
}

fn _borrow<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut state = State::read(&deps.storage)?;
    let mut position = Position::read_or_new(&deps.storage, &user_raw)?;

    let response = if !amount.is_zero() {
        // Total amount of short asset owed by the contract to Mars
        let total_debt = config.red_bank.query_debt(
            deps,
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
        let amount_after_tax = config.short_asset.deduct_tax(deps, amount)?;

        // Update storage
        state.total_debt_units += debt_units_to_add;
        state.write(&mut deps.storage)?;

        // Update position
        position.debt_units += debt_units_to_add;
        position.unlocked_assets[1].amount += amount_after_tax;
        position.write(&mut deps.storage, &user_raw)?;

        // Generate message
        let borrow_message = config.red_bank.borrow_message(&Asset {
            info: config.short_asset.clone(),
            amount,
        })?;

        HandleResponse {
            messages: vec![borrow_message],
            log: vec![
                log("action", "martial_field::CallbackMsg::Borrow"),
                log("user", user),
                log("amount", amount),
                log("amount_after_tax", amount_after_tax),
                log("debt_units_added", debt_units_to_add),
            ],
            data: None,
        }
    } else {
        HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "martian_field::CallbackMsg::Borrow"),
                log("warning", "skipped: borrow amount is zero!"),
            ],
            data: None,
        }
    };

    Ok(response)
}

fn _repay<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = Config::read_normal(deps)?;
    let mut state = State::read(&deps.storage)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Amount of short asset to repay
    let amount = position.unlocked_assets[1].amount;

    let response = if !amount.is_zero() {
        // Total amount of short asset owed by the contract to Mars
        let total_debt = config.red_bank.query_debt(
            deps,
            &env.contract.address,
            &config.short_asset,
        )?;

        // Amount of debt assigned to the user
        let debt_amount =
            total_debt.multiply_ratio(position.debt_units, state.total_debt_units);

        // Due to tax, the amount of `repay_asset` received may not be fully delivered to
        // Mars. Calculate the maximum deliverable amount.
        let amount_after_tax = config.short_asset.deduct_tax(deps, amount)?;

        // If the user pays more than what he owes, we reduce his debt units to zero.
        // Otherwise, we reduce his debt units proportionately.
        let debt_units_to_reduce = std::cmp::min(
            position.debt_units,
            position.debt_units.multiply_ratio(amount_after_tax, debt_amount),
        );

        // Update state
        state.total_debt_units = (state.total_debt_units - debt_units_to_reduce)?;
        state.write(&mut deps.storage)?;

        // Update position
        position.debt_units = (position.debt_units - debt_units_to_reduce)?;
        position.unlocked_assets[1].amount = Uint128::zero();
        position.write(&mut deps.storage, &user_raw)?;

        // Generate message
        let repay_message = config.red_bank.repay_message(&Asset {
            info: config.short_asset.clone(),
            amount: amount_after_tax,
        })?;

        HandleResponse {
            messages: vec![repay_message],
            log: vec![
                log("action", "martian_field::CallbackMsg::Repay"),
                log("user", user),
                log("amount", amount),
                log("amount_after_tax", amount_after_tax),
                log("debt_units_reduced", debt_units_to_reduce),
            ],
            data: None,
        }
    } else {
        HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "martian_field::CallbackMsg::Repay"),
                log("warning", "skipped: repay amount is zero!"),
            ],
            data: None,
        }
    };

    Ok(response)
}

fn _swap<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let contract_addr_raw = deps.api.canonical_address(&env.contract.address)?;
    let config = Config::read_normal(deps)?;
    let mut position = Position::read_or_new(&deps.storage, &contract_addr_raw)?;

    // Calculate how much performance fee should be charged
    let fee = amount * config.fee_rate;
    let amount_after_fee = (amount - fee)?;

    // Half of the reward is to be retained, not swapped
    let retain_amount = amount_after_fee * Decimal::from_ratio(1u128, 2u128);

    // The amount of reward to be swapped
    // Note: here we assume `long_token` == `reward_token`. This is the case for popular
    // farms e.g. ANC, MIR, MINE, but not for mAsset farms.
    // MAsset support may be added in a future version
    let offer_amount = (amount_after_fee - retain_amount)?;
    let offer_amount_after_tax = config.long_asset.deduct_tax(deps, offer_amount)?;

    // Note: must deduct tax here
    let offer_asset = Asset {
        info: config.long_asset.clone(),
        amount: offer_amount_after_tax,
    };

    // Calculate the return amount of the swap
    // Note: must deduct_tax here
    let return_amount = config.swap.simulate_swap(deps, &offer_asset)?;
    let return_amount_after_tax = config.short_asset.deduct_tax(deps, return_amount)?;

    // Update position
    position.unlocked_assets[0].amount += retain_amount;
    position.unlocked_assets[1].amount += return_amount_after_tax;
    position.write(&mut deps.storage, &contract_addr_raw)?;

    Ok(HandleResponse {
        messages: vec![
            config.long_asset.transfer_message(
                deps,
                &env.contract.address,
                &config.treasury,
                fee,
            )?,
            config.swap.swap_message(&offer_asset)?,
        ],
        log: vec![
            log("action", "martian_field::CallbackMsg::Swap"),
            log("amount", amount),
            log("fee_amount", fee),
            log("retain_amount", retain_amount),
            log("offer_amount", offer_amount),
            log("offer_after_tax", offer_amount_after_tax),
            log("return_amount", return_amount),
            log("return_after_tax", return_amount_after_tax),
        ],
        data: None,
    })
}

fn _refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    user: HumanAddr,
    recipient: HumanAddr,
    percentage: Decimal,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let mut position = Position::read(&deps.storage, &user_raw)?;

    // Apply percentage
    let assets: Vec<Asset> = position
        .unlocked_assets
        .to_vec()
        .iter()
        .map(|asset| Asset {
            info: asset.info.to_normal(deps).unwrap(),
            amount: asset.amount * percentage,
        })
        .collect();

    // Update position
    for i in 0..2 {
        position.unlocked_assets[i].amount =
            (position.unlocked_assets[i].amount - assets[i].amount)?;
    }
    position.write(&mut deps.storage, &user_raw)?;

    // Generate messages for the transfers
    // Notes:
    // 1. Must filter off assets whose amounts are zero
    // 2. `asset.transfer_message` does tax deduction so no need to do it here
    let messages = assets
        .iter()
        .filter(|asset| !asset.amount.is_zero())
        .map(|asset| asset.transfer_message(deps, &user, &recipient).unwrap())
        .collect();

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "martian_field::CallbackMsg::Refund"),
            log("user", user),
            log("recipient", recipient),
            log("percentage", percentage),
            log("long_refunded", assets[0].amount),
            log("short_refunded", assets[1].amount),
            log("shares_refunded", assets[2].amount),
        ],
        data: None,
    })
}

fn _snapshot<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let position = Position::read(&deps.storage, &user_raw)?;

    let snapshot = Snapshot {
        time: env.block.time,
        height: env.block.height,
        health: query_health(&deps, Some(user.clone()))?,
        position,
    };

    snapshot.write(&mut deps.storage, &user_raw)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "martian_field::CallbackMsg::Snapshot"),
            log("user", user),
        ],
        data: None,
    })
}

fn _purge<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let position = Position::read(&deps.storage, &user_raw)?;

    if position.is_empty() {
        Position::delete(&mut deps.storage, &user_raw);
        Snapshot::delete(&mut deps.storage, &user_raw);
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "martian_field::CallbackMsg::Purge"),
            log("user", user),
            log("purged", position.is_empty()),
        ],
        data: None,
    })
}

fn _assert_health<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let config_raw = Config::read(&deps.storage)?;
    let health_info = query_health(deps, Some(user.clone()))?;

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
        Ok(HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "martian_field::CallbackMsg::AssertHealth"),
                log("user", user),
                log("ltv", ltv_str),
            ],
            data: None,
        })
    } else {
        Err(StdError::generic_err("LTV is greater than liquidation threshold!"))
    }
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    Config::read(&deps.storage)?.to_response(&deps)
}

fn query_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<StateResponse> {
    State::read(&deps.storage)?.to_response()
}

fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: HumanAddr,
) -> StdResult<PositionResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    Position::read(&deps.storage, &user_raw)?.to_response(&deps)
}

fn query_snapshot<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: HumanAddr,
) -> StdResult<SnapshotResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    Snapshot::read(&deps.storage, &user_raw)?.to_response(&deps)
}

fn query_health<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: Option<HumanAddr>,
) -> StdResult<HealthResponse> {
    let config = Config::read_normal(deps)?;
    let state = State::read(&deps.storage)?;
    let contract_addr = deps.api.human_address(&state.contract_addr)?;

    let (bond_units, debt_units) = if let Some(user) = user {
        let user_raw = deps.api.canonical_address(&user)?;
        let position = Position::read(&deps.storage, &user_raw)?;
        (position.bond_units, position.debt_units)
    } else {
        (state.total_bond_units, state.total_debt_units)
    };

    // Info of the TerraSwap pool
    let pool_info =
        config.swap.query_pool(&deps, &config.long_asset, &config.short_asset)?;

    // Total amount of debt owed to Mars
    let total_debt =
        config.red_bank.query_debt(&deps, &contract_addr, &config.short_asset)?;

    // Total amount of share tokens bonded in the staking contract
    let total_bond = config.staking.query_bond(&deps, &contract_addr)?;

    // Value of each units of share, measured in the short asset
    // Note: Here we don't check whether `pool_info.share_supply` is zero here because
    // in practice it should never be zero
    let share_value = Decimal::from_ratio(
        pool_info.short_depth + pool_info.short_depth,
        pool_info.share_supply,
    );

    // Amount of bonded shares assigned to the user
    // Note: must handle division by zero!
    let bond_amount = if state.total_bond_units.is_zero() {
        Uint128::zero()
    } else {
        total_bond.multiply_ratio(bond_units, state.total_bond_units)
    };

    // Value of bonded shares assigned to the user
    let bond_value = bond_amount * share_value;

    // Value of debt assigned to the user
    // Note: must handle division by zero!
    let debt_value = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_debt.multiply_ratio(debt_units, state.total_debt_units)
    };

    // Loan-to-value ratio
    // Note: must handle division by zero!
    // `bond_units` can be zero if the position has been closed, pending liquidation
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
