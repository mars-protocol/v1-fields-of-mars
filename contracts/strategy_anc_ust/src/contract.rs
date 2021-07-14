use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{
    log, to_binary, Api, BankMsg, Binary, Coin, CosmosMsg, Decimal, Env, Extern,
    HandleResponse, HumanAddr, InitResponse, MigrateResponse, Querier, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};
use cw20::Cw20HandleMsg;
use mars::liquidity_pool as mars;
use terraswap::{
    asset::{Asset, AssetInfo},
    querier::{query_balance, query_supply, query_token_balance},
};

use fields_of_mars::strategy_anc_ust::{
    CallbackMsg, ConfigResponse, HandleMsg, InitMsg, MigrateMsg, PositionResponse,
    QueryMsg, StateResponse,
};

use crate::{
    helpers::{
        add_tax, compute_ltv, compute_swap_return_amount, deduct_tax, parse_ust_received,
        query_debt_amount,
    },
    staking::StakingContract,
    state::{
        delete_position, read_config, read_position, read_state, write_config,
        write_position, write_state, Config, Position, State,
    },
};

//----------------------------------------------------------------------------------------
// ENTRY POINTS
//----------------------------------------------------------------------------------------

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    write_state(
        &mut deps.storage,
        &State::new(deps.api.canonical_address(&env.contract.address)?),
    )?;
    Ok(InitResponse {
        messages: vec![CallbackMsg::UpdateConfig {
            new_config: msg,
        }
        .into_cosmos_msg(&env.contract.address)?],
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::IncreasePosition {
            asset_amount,
        } => increase_position(deps, env, asset_amount),
        HandleMsg::ReducePosition {
            bond_units,
        } => reduce_position(deps, env, bond_units),
        HandleMsg::PayDebt {
            user,
        } => pay_debt(deps, env, user),
        HandleMsg::Liquidate {
            user,
        } => liquidate(deps, env, user),
        HandleMsg::Harvest {} => harvest(deps, env),
        HandleMsg::UpdateConfig {
            new_config,
        } => update_config(deps, env, new_config),
        HandleMsg::Callback(callback_msg) => _handle_callback(deps, env, callback_msg),
    }
}

pub fn _handle_callback<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    callback_msg: CallbackMsg,
) -> StdResult<HandleResponse> {
    // Callback functions can only be called this contract itself
    if env.message.sender != env.contract.address {
        return Err(StdError::unauthorized());
    }
    match callback_msg {
        CallbackMsg::ProvideLiquidity {
            asset_amount,
            ust_amount,
            user,
        } => _provide_liquidity(deps, env, asset_amount, ust_amount, user),
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
            borrow_amount,
        } => _borrow(deps, env, user, borrow_amount),
        CallbackMsg::Repay {
            user,
            repay_amount,
        } => _repay(deps, env, user, repay_amount),
        CallbackMsg::SwapReward {
            reward_amount,
        } => _swap_reward(deps, env, reward_amount),
        CallbackMsg::Refund {
            user,
        } => _refund(deps, env, user),
        CallbackMsg::ClaimCollateral {
            user,
            liquidator,
            repay_amount,
        } => _claim_collateral(deps, env, user, liquidator, repay_amount),
        CallbackMsg::UpdateConfig {
            new_config,
        } => _update_config(deps, env, new_config),
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
// HANDLE FUNCTIONS
//----------------------------------------------------------------------------------------

/**
 * @notice Open a new position or add to an existing position.
 *
 * @dev The user must have approved the strategy to draw MIR tokens from his wallet.
 * Typically, `Cw20HandleMsg::Approve` and this message are sent along in the same
 * transaction.
 *
 * @dev Any amount of UST may be sent along with the message. The strategy calculates how
 * much UST is needed for liquidity provision; if the amount send by the user is not
 * sufficient, borrows uncollateralized loan from Mars to make up the difference.
 *
 * @dev The strategy does not check if there is enough UST liquidity at Mars to be  
 * borrowed, or if it has enough credit line for borrowing uncollateralized loans. The
 * frontend should perform these checks.
 */
pub fn increase_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_to_draw: Uint128,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;
    let pool = deps.api.human_address(&config.pool)?;

    // Query UST and asset balances of the Terraswap pool
    let pool_ust = query_balance(deps, &pool, "uusd".to_string())?;
    let pool_asset = query_token_balance(deps, &asset_token, &pool)?;

    // Calculate how much UST is need for liquidity provision
    // We don't check whether `pool_asset` is zero here because we know it's not
    let ust_received = parse_ust_received(&env.message);
    let ust_needed = pool_ust.multiply_ratio(asset_to_draw, pool_asset);

    // Calculate how much UST to borrow from Mars
    let ust_to_borrow = if ust_needed > ust_received {
        (ust_needed - ust_received)?
    } else {
        Uint128::zero()
    };

    // Calculate how much UST to provide to Terraswap.
    // Note: If borrowing from Mars, the actual received amount is smaller than the borrow
    // amount, dut to tax. This amount must be deducted
    let ust_to_provide = if ust_to_borrow.is_zero() {
        ust_received
    } else {
        ust_received + deduct_tax(deps, ust_to_borrow, "uusd")?
    };

    let mut messages = if !ust_to_borrow.is_zero() {
        vec![CallbackMsg::Borrow {
            user: env.message.sender.clone(),
            borrow_amount: ust_to_borrow,
        }
        .into_cosmos_msg(&env.contract.address)?]
    } else {
        vec![]
    };

    messages.extend(vec![
        CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: asset_token,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::TransferFrom {
                owner: env.message.sender.clone(),
                recipient: env.contract.address.clone(),
                amount: asset_to_draw,
            })?,
        }),
        CallbackMsg::ProvideLiquidity {
            asset_amount: asset_to_draw,
            ust_amount: ust_to_provide,
            user: Some(env.message.sender),
        }
        .into_cosmos_msg(&env.contract.address)?,
    ]);

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "handle: increase_position"),
            log("asset_received", asset_to_draw),
            log("ust_received", ust_received),
        ],
        data: None,
    })
}

/**
 * @notice Reduce a position, or close it completely.
 *
 * @dev The resulting debt ratio must be less or equal than the liquidation threshold, or
 * the transaction reverts.
 *
 * @dev Callable only by the user himself.
 */
pub fn reduce_position<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    bond_units: Option<Uint128>,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&env.message.sender)?;
    let position = read_position(&deps.storage, &user_raw)?;

    // If parameter `bond_units` is not provided, then remove all
    let bond_units_to_reduce = if let Some(bond_units) = bond_units {
        bond_units
    } else {
        position.bond_units
    };

    Ok(HandleResponse {
        messages: vec![
            CallbackMsg::Unbond {
                user: env.message.sender.clone(),
                bond_units: bond_units_to_reduce,
            }
            .into_cosmos_msg(&env.contract.address)?,
            CallbackMsg::Refund {
                user: env.message.sender,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ],
        log: vec![
            log("action", "handle: reduce_position"),
            log("bond_units_reduced", bond_units_to_reduce),
        ],
        data: None,
    })
}

/**
 * @notice Pay down debt owed to Mars, reduce debt units.
 *
 * @dev Stability fee (also known as "tax") is charged twice during this function's
 * execution:
 *
 * 1) during the transfer of UST from the user's wallet to the strategy,
 * 2) from the strategy to Mars.
 *
 * Among these, 1) is directly deducted from the user's wallet balance, while 2) is
 * deducted the the strategy's balance. The frontend should handle this.
 *
 * @dev For example, if a user wishes to pay down 100 UST debt, and the tax for a 100 UST
 * transfer is 0.1 UST, he needs to actually send 100.1 UST to the contract, of which only
 * 100 UST will be delivered to Mars, and a total of 100.2 UST (plus gas fee) will be
 * deducted from his account.
 */
pub fn pay_debt<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    env: Env,
    user: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let ust_received = parse_ust_received(&env.message);

    // If `user` parameter is not provided, then pay down the message sender's debt
    let user = if let Some(user) = user {
        user
    } else {
        env.message.sender
    };

    Ok(HandleResponse {
        messages: vec![CallbackMsg::Repay {
            user,
            repay_amount: ust_received,
        }
        .into_cosmos_msg(&env.contract.address)?],
        log: vec![log("action", "handle: pay_debt")],
        data: None,
    })
}

/**
 * @notice Claim staking reward and reinvest.
 *
 * @dev For now, only the owner is allowed to call this function. Later, once the strategy
 * is proven to be stable, we will consider making this function callable by anyone.
 */
pub fn harvest<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let sender_raw = deps.api.canonical_address(&env.message.sender)?;
    let config = read_config(&deps.storage)?;
    let staking_contract = StakingContract::from_config(deps, &config)?;

    // Only owner and whitelisted operators can call harvest
    if sender_raw != config.owner
        && config.operators.iter().all(|operator| sender_raw != operator.clone())
    {
        return Err(StdError::unauthorized());
    }

    // Query the amount of MIR reward to expect to receive
    let reward_amount =
        staking_contract.query_reward_amount(deps, &env.contract.address)?;

    Ok(HandleResponse {
        messages: vec![
            staking_contract.withdraw_message()?,
            CallbackMsg::SwapReward {
                reward_amount,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ],
        log: vec![log("action", "handle: harvest"), log("reward_amount", reward_amount)],
        data: None,
    })
}

/**
 * @notice Close an underfunded position, pay down remaining debt and claim the collateral.
 *
 * @dev Callable by anyone, but only if the position's debt ratio is above the liquidation
 * threshold.
 *
 * @dev If the position is active (defined by `bond_units` > 0), the position will first
 * be closed. This involves unbonding the LP tokens from Mirror Staking contract, remove
 * liquidity from TerraSwap, use the UST proceedings to pay off the debt, and withhold the
 * MIR proceedings pending liquidation. At this time, anyone can send along UST to pay off
 * a portion of the remaining debt, and being awarded a portion of the withheld MIR.
 */
pub fn liquidate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let ust_received = parse_ust_received(&env.message);
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let position = read_position(&deps.storage, &user_raw)?;

    // The position must be closed, or has a debt ratio above the liquidation threshold
    // Note: If the position is closed, `compute_debt_ratio` should return `None` for
    // `debt_ratio`. Therefore we only need to verify when `debt_ratio` is not None.
    let (.., ltv) = compute_ltv(deps, Some(user.clone()))?;
    if let Some(ltv) = ltv {
        if ltv <= config.max_ltv {
            return Err(StdError::generic_err("cannot liquidate a healthy position"));
        }
    }

    // If the position is open, we close it first, and use the proceeding to pay back debt
    let mut messages = if position.is_active() {
        vec![CallbackMsg::Unbond {
            user: user.clone(),
            bond_units: position.bond_units,
        }
        .into_cosmos_msg(&env.contract.address)?]
    } else {
        vec![]
    };

    messages.push(
        CallbackMsg::ClaimCollateral {
            user,
            liquidator: env.message.sender,
            repay_amount: ust_received,
        }
        .into_cosmos_msg(&env.contract.address)?,
    );

    Ok(HandleResponse {
        messages,
        log: vec![log("action", "handle: liquidate"), log("ust_received", ust_received)],
        data: None,
    })
}

/**
 * @notice Update data stored in config.
 */
pub fn update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    new_config: InitMsg,
) -> StdResult<HandleResponse> {
    // Only contract owner can call this message
    let config = read_config(&deps.storage)?;
    if env.message.sender != deps.api.human_address(&config.owner)? {
        return Err(StdError::unauthorized());
    }

    Ok(HandleResponse {
        messages: vec![CallbackMsg::UpdateConfig {
            new_config,
        }
        .into_cosmos_msg(&env.contract.address)?],
        log: vec![log("action", "handle: update_config")],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// CALLBACK FUNCTIONS
//----------------------------------------------------------------------------------------

/**
 * @notice Provide specified amounts of MIR and UST to the Terraswap pool, receive LP tokens.
 */
pub fn _provide_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset_amount: Uint128,
    ust_amount: Uint128,
    user: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;
    let pool = deps.api.human_address(&config.pool)?;

    // Sending native tokens such as UST involves a tax (stability fee), therefore the
    // amount can't be delivered in full. Calculate how much tax will be charged, and
    // the actual deliverable amount.
    let ust_amount_after_tax = deduct_tax(deps, ust_amount, "uusd")?;

    Ok(HandleResponse {
        messages: vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::IncreaseAllowance {
                    spender: pool.clone(),
                    amount: asset_amount,
                    expires: None,
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pool,
                send: vec![Coin {
                    denom: "uusd".to_string(),
                    amount: ust_amount_after_tax,
                }],
                msg: to_binary(&terraswap::pair::HandleMsg::ProvideLiquidity {
                    assets: [
                        Asset {
                            info: AssetInfo::NativeToken {
                                denom: "uusd".to_string(),
                            },
                            amount: ust_amount_after_tax,
                        },
                        Asset {
                            info: AssetInfo::Token {
                                contract_addr: asset_token,
                            },
                            amount: asset_amount,
                        },
                    ],
                    slippage_tolerance: None,
                })?,
            }),
            CallbackMsg::Bond {
                user,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ],
        log: vec![
            log("action", "callback: provide_liquidity"),
            log("asset_provided", asset_amount),
            log("ust_provided", ust_amount_after_tax),
        ],
        data: None,
    })
}

/**
 * @notice Burn LP tokens, remove the liquidity from Terraswap, receive MIR and UST.
 */
pub fn _remove_liquidity<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let state = read_state(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;
    let pool = deps.api.human_address(&config.pool)?;
    let pool_token = deps.api.human_address(&config.pool_token)?;

    // Find the amount of LP tokens the contract has received from unstaking
    let pool_tokens_to_burn =
        query_token_balance(deps, &pool_token, &env.contract.address)?;

    // Query info related to the Terraswap pair
    let pool_ust = query_balance(deps, &pool, "uusd".to_string())?;
    let pool_asset = query_token_balance(deps, &asset_token, &pool)?;
    let pool_token_supply = query_supply(deps, &pool_token)?;

    // Calculate how much asset will be released. Logic is copied from:
    // terraswap/terraswap/contracts/pool/src/contract.rs#L294
    let ust_to_be_released =
        pool_ust * Decimal::from_ratio(pool_tokens_to_burn, pool_token_supply);
    let asset_to_be_released =
        pool_asset * Decimal::from_ratio(pool_tokens_to_burn, pool_token_supply);

    // Due to tax, the receivable UST amount from liquidity removal is slightly less
    let ust_to_receive = deduct_tax(&deps, ust_to_be_released, "uusd")?;

    // Calculate the amount of UST the user owes to Mars
    // Note: Must handle division of zero!
    let total_ust_owed = query_debt_amount(&deps, &env.contract.address)?;
    let ust_owed = if state.total_debt_units.is_zero() {
        Uint128::zero()
    } else {
        total_ust_owed.multiply_ratio(position.debt_units, state.total_debt_units)
    };

    // Find out how much UST to be used to pay debt, and how much to be refunded to user.
    // Note: The actual UST amount needed to fully pay down debt is the debt amount + tax
    let ust_to_repay = std::cmp::min(ust_to_receive, add_tax(&deps, ust_owed, "uusd")?);
    let ust_to_refund = if ust_to_repay < ust_to_receive {
        (ust_to_receive - ust_to_repay)?
    } else {
        Uint128::zero()
    };

    // Increment the user's unstaked token, UST amounts
    position.unbonded_asset_amount += asset_to_be_released;
    position.unbonded_ust_amount += ust_to_refund;
    write_position(&mut deps.storage, &user_raw, &position)?;

    let mut messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pool_token.clone(),
        send: vec![],
        msg: to_binary(&Cw20HandleMsg::Send {
            contract: pool,
            amount: pool_tokens_to_burn,
            msg: Some(to_binary(&terraswap::pair::Cw20HookMsg::WithdrawLiquidity {})?),
        })?,
    })];

    if !ust_to_repay.is_zero() {
        messages.push(
            CallbackMsg::Repay {
                user: user.clone(),
                repay_amount: ust_to_repay,
            }
            .into_cosmos_msg(&env.contract.address)?,
        );
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "callback: remove_liquidity"),
            log("pool_tokens_burned", pool_tokens_to_burn),
        ],
        data: None,
    })
}

/**
 * Bond LP tokens to Mirror Staking contract.
 */
pub fn _bond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: Option<HumanAddr>,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let mut state = read_state(&deps.storage)?;
    let pool_token = deps.api.human_address(&config.pool_token)?;
    let staking_contract = StakingContract::from_config(deps, &config)?;

    // Find the amount of LP tokens already bonded to the staking contract
    let total_bond_amount =
        staking_contract.query_bond_amount(deps, &env.contract.address)?;

    // Find the amount of LP tokens the contract has received from liquidity provision
    let amount_to_bond = query_token_balance(deps, &pool_token, &env.contract.address)?;

    // If a user account is provided, then increment the asset units
    // Initial asset unit = 100,000 units per LP token staked
    let bond_units_to_add = if !user.is_none() {
        if total_bond_amount.is_zero() {
            amount_to_bond.multiply_ratio(1_000_000u128, 1u128)
        } else {
            state.total_bond_units.multiply_ratio(amount_to_bond, total_bond_amount)
        }
    } else {
        Uint128::zero()
    };

    // Update storage
    // If the user doesn't have a position yet, we initialize a new one
    if !bond_units_to_add.is_zero() {
        let user_raw = deps.api.canonical_address(&user.unwrap())?;
        let mut position = match read_position(&deps.storage, &user_raw) {
            Ok(position) => position,
            Err(_) => Position::default(),
        };
        state.total_bond_units += bond_units_to_add;
        write_state(&mut deps.storage, &state)?;

        position.bond_units += bond_units_to_add;
        write_position(&mut deps.storage, &user_raw, &position)?;
    }

    Ok(HandleResponse {
        messages: vec![staking_contract.bond_message(amount_to_bond)?],
        log: vec![
            log("action", "callback: bond"),
            log("amount_bonded", amount_to_bond),
            log("bond_units_added", bond_units_to_add),
        ],
        data: None,
    })
}

/**
 * @notice Unbond LP tokens from Mirror Staking contract.
 */
pub fn _unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    bond_units: Uint128,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let mut state = read_state(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw)?;
    let staking_contract = StakingContract::from_config(deps, &config)?;

    // Calculate how many LP tokens should be unbonded
    let total_bond_amount =
        staking_contract.query_bond_amount(deps, &env.contract.address)?;
    let amount_to_unbond =
        total_bond_amount.multiply_ratio(bond_units, state.total_bond_units);

    // Reduce the asset units
    state.total_bond_units = (state.total_bond_units - bond_units)?;
    write_state(&mut deps.storage, &state)?;

    position.bond_units = (position.bond_units - bond_units)?;
    write_position(&mut deps.storage, &user_raw, &position)?;

    Ok(HandleResponse {
        messages: vec![
            staking_contract.unbond_message(amount_to_unbond)?,
            CallbackMsg::RemoveLiquidity {
                user,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ],
        log: vec![
            log("action", "callback: unbond"),
            log("amount_unbonded", amount_to_unbond),
        ],
        data: None,
    })
}

/**
 * @notice Borrow UST as uncollateralized loan from Mars.
 */
pub fn _borrow<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    borrow_amount: Uint128,
) -> StdResult<HandleResponse> {
    if borrow_amount.is_zero() {
        return Err(StdError::generic_err("borrow amount must be greater than zero"));
    }

    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let mut state = read_state(&deps.storage)?;
    let mars = deps.api.human_address(&config.mars)?;

    // If the user doesn't have a position yet, we initialize a new one
    let mut position = match read_position(&deps.storage, &user_raw) {
        Ok(position) => position,
        Err(_) => Position::default(),
    };

    let total_debt_amount = query_debt_amount(&deps, &env.contract.address)?;

    // Calculate how many debt units the user should be accredited
    // We define the initial debt unit = 100,000 units per UST borrowed
    let debt_units_to_add = if total_debt_amount.is_zero() {
        borrow_amount.multiply_ratio(1_000_000u128, 1u128)
    } else {
        state.total_debt_units.multiply_ratio(borrow_amount, total_debt_amount)
    };

    // Update storage
    state.total_debt_units += debt_units_to_add;
    write_state(&mut deps.storage, &state)?;

    position.debt_units += debt_units_to_add;
    write_position(&mut deps.storage, &user_raw, &position)?;

    Ok(HandleResponse {
        messages: vec![WasmMsg::Execute {
            contract_addr: mars,
            send: vec![],
            msg: to_binary(&mars::HandleMsg::Borrow {
                asset: mars::Asset::Native {
                    denom: "uusd".to_string(),
                },
                amount: Uint256::from(borrow_amount),
            })?,
        }
        .into()],
        log: vec![
            log("action", "callback: borrow"),
            log("amount_borrowed", borrow_amount),
            log("debt_units_added", debt_units_to_add),
        ],
        data: None,
    })
}

/**
 * @notice Pay specified amount of UST to Mars.
 */
pub fn _repay<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    repay_amount: Uint128,
) -> StdResult<HandleResponse> {
    if repay_amount.is_zero() {
        return Err(StdError::generic_err("repay amount must be greater than zero"));
    }

    let user_raw = deps.api.canonical_address(&user)?;
    let mut state = read_state(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw)?;

    if position.debt_units.is_zero() {
        return Err(StdError::generic_err("no debt to repay"));
    }

    let mars = deps.api.human_address(&read_config(&deps.storage)?.mars)?;
    let total_debt_amount = query_debt_amount(&deps, &env.contract.address)?;

    // The amount of UST owed by the user
    let ust_owed =
        total_debt_amount.multiply_ratio(position.debt_units, state.total_debt_units);

    // Due to tax, the amount of UST received cannot be fully delivered to Mars. Here we
    // calculate the maximum deliverable amount.
    let ust_to_repay = deduct_tax(deps, repay_amount, "uusd")?;

    // If the user pays more than what he owes, we reduce his debt units to zero.
    // Otherwise, we reduce his debt units proportionatelly.
    let debt_units_to_reduce = if ust_to_repay > total_debt_amount {
        position.debt_units
    } else {
        position.debt_units.multiply_ratio(ust_to_repay, ust_owed)
    };

    // Update state
    state.total_debt_units = (state.total_debt_units - debt_units_to_reduce)?;
    write_state(&mut deps.storage, &state)?;

    // Update position. Delete if empty
    position.debt_units = (position.debt_units - debt_units_to_reduce)?;
    if position.is_empty() {
        delete_position(&mut deps.storage, &user_raw);
    } else {
        write_position(&mut deps.storage, &user_raw, &position)?;
    };

    Ok(HandleResponse {
        messages: vec![WasmMsg::Execute {
            contract_addr: mars,
            send: vec![Coin {
                denom: "uusd".to_string(),
                amount: ust_to_repay,
            }],
            msg: to_binary(&mars::HandleMsg::RepayNative {
                denom: "uusd".to_string(),
            })?,
        }
        .into()],
        log: vec![
            log("action", "callback: repay"),
            log("ust_received", repay_amount),
            log("ust_repaid", ust_to_repay),
            log("debt_units_reduced", debt_units_to_reduce),
            log("position_deleted", position.is_empty()),
        ],
        data: None,
    })
}

/**
 * @notice Collect a portion of rewards as performance fee, swap half of the rest for UST.
 */
pub fn _swap_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    reward_amount: Uint128,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;
    let treasury = deps.api.human_address(&config.treasury)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;
    let reward_token = deps.api.human_address(&config.reward_token)?;
    let pool = deps.api.human_address(&config.pool)?;

    // Calculate how much performance fee should be charged
    let fee_amount = reward_amount * config.performance_fee_rate;
    let reward_amount_after_fee = (reward_amount - fee_amount)?;

    let mut messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: asset_token.clone(),
        send: vec![],
        msg: to_binary(&Cw20HandleMsg::Transfer {
            recipient: treasury,
            amount: fee_amount,
        })?,
    })];

    if asset_token == reward_token {
        // Half of the after-fee reward to be swapped for UST, the other half to be
        // provided to Terraswap pool
        let reward_to_swap = reward_amount_after_fee * Decimal::from_ratio(1u128, 2u128);
        let asset_to_provide = (reward_amount_after_fee - reward_to_swap)?;

        // Calculate how many UST can to expect from the swap
        let pool_ust = query_balance(deps, &pool, "uusd".to_string())?;
        let pool_asset = query_token_balance(deps, &asset_token, &pool)?;
        let ust_to_provide =
            compute_swap_return_amount(deps, reward_to_swap, pool_asset, pool_ust)?;

        messages.extend(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: asset_token.clone(),
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Send {
                    contract: pool.clone(),
                    amount: reward_to_swap,
                    msg: Some(to_binary(&terraswap::pair::HandleMsg::Swap {
                        offer_asset: Asset {
                            info: AssetInfo::Token {
                                contract_addr: asset_token,
                            },
                            amount: reward_to_swap,
                        },
                        belief_price: None,
                        max_spread: None,
                        to: None,
                    })?),
                })?,
            }),
            CallbackMsg::ProvideLiquidity {
                asset_amount: asset_to_provide,
                ust_amount: ust_to_provide,
                user: None,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ])
    } else {
        // We currently only support case where asset_token and reward_token are the same
        return Err(StdError::generic_err("unimplemented"));
    };

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "callback: swap_reward"),
            log("fee_amount", fee_amount),
            log("reward_amount_after_fee", reward_amount_after_fee),
        ],
        data: None,
    })
}

/**
 * @notice Verify the user's debt ratio, then refund unstaked MIR and UST to the user.
 */
pub fn _refund<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw)?;

    // Calculate debt ratio, make sure it's below the liquidation threshold
    let (.., debt_value, ltv) = compute_ltv(deps, Some(user.clone()))?;
    if !debt_value.is_zero() && ltv.unwrap() > config.max_ltv {
        return Err(StdError::generic_err("LTV above liquidation threshold"));
    }

    // Calculate the amount of UST and asset to refund
    let ust_to_refund = deduct_tax(&deps, position.unbonded_ust_amount, "uusd")?;
    let asset_to_refund = position.unbonded_asset_amount;

    position.unbonded_ust_amount = Uint128::zero();
    position.unbonded_asset_amount = Uint128::zero();

    // Delete the user's position if there is no asset and debt left
    if position.is_empty() {
        delete_position(&mut deps.storage, &user_raw);
    } else {
        write_position(&mut deps.storage, &user_raw, &position)?;
    };

    let mut messages: Vec<CosmosMsg> = vec![];

    if !ust_to_refund.is_zero() {
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address,
            to_address: user.clone(),
            amount: vec![Coin {
                denom: "uusd".to_string(),
                amount: ust_to_refund,
            }],
        }));
    }

    if !asset_to_refund.is_zero() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.asset_token)?,
            send: vec![],
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: user,
                amount: asset_to_refund,
            })?,
        }));
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "callback: refund"),
            log("asset_refunded", asset_to_refund),
            log("ust_refunded", ust_to_refund),
            log("ltv", ltv.unwrap_or_default()),
            log("position_deleted", position.is_empty()),
        ],
        data: None,
    })
}

/**
 * @notice Receive UST, pay back debt, and credit the liquidator a share of the collateral.
 */
pub fn _claim_collateral<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    user: HumanAddr,
    liquidator: HumanAddr,
    repay_amount: Uint128,
) -> StdResult<HandleResponse> {
    let user_raw = deps.api.canonical_address(&user)?;
    let config = read_config(&deps.storage)?;
    let state = read_state(&deps.storage)?;
    let mut position = read_position(&deps.storage, &user_raw)?;
    let asset_token = deps.api.human_address(&config.asset_token)?;

    // Due to tax, the amount of UST received cannot be fully delivered to Mars
    let repay_amount_after_tax = deduct_tax(deps, repay_amount, "uusd")?;

    // Calculate how much tax the user owes
    // Note: we don't need to check for division by zero, as we intend this to fail if
    // there is no debt to pay.
    let total_debt_amount = query_debt_amount(&deps, &env.contract.address)?;
    let debt_amount =
        total_debt_amount.multiply_ratio(position.debt_units, state.total_debt_units);

    // Calculate how much unstaked asset should be accredited to the liquidator
    let asset_to_release = if repay_amount_after_tax > debt_amount {
        position.unbonded_asset_amount
    } else {
        position.unbonded_asset_amount.multiply_ratio(repay_amount_after_tax, debt_amount)
    };

    // Update storage
    position.unbonded_asset_amount = (position.unbonded_asset_amount - asset_to_release)?;
    write_position(&mut deps.storage, &user_raw, &position)?;

    Ok(HandleResponse {
        messages: vec![
            WasmMsg::Execute {
                contract_addr: asset_token,
                send: vec![],
                msg: to_binary(&Cw20HandleMsg::Transfer {
                    recipient: liquidator,
                    amount: asset_to_release,
                })?,
            }
            .into(),
            CallbackMsg::Repay {
                user,
                repay_amount,
            }
            .into_cosmos_msg(&env.contract.address)?,
        ],
        log: vec![
            log("action", "callback: claim_collateral"),
            log("asset_released", asset_to_release),
        ],
        data: None,
    })
}

/**
 * @notice Update data stored in config.
 */
pub fn _update_config<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    config: InitMsg,
) -> StdResult<HandleResponse> {
    let operators_raw = config
        .operators
        .iter()
        .map(|operator| deps.api.canonical_address(&operator).unwrap())
        .collect();

    write_config(
        &mut deps.storage,
        &Config {
            owner: deps.api.canonical_address(&config.owner)?,
            operators: operators_raw,
            treasury: deps.api.canonical_address(&config.treasury)?,
            asset_token: deps.api.canonical_address(&config.asset_token)?,
            reward_token: deps.api.canonical_address(&config.reward_token)?,
            pool: deps.api.canonical_address(&config.pool)?,
            pool_token: deps.api.canonical_address(&config.pool_token)?,
            mars: deps.api.canonical_address(&config.mars)?,
            staking_contract: deps.api.canonical_address(&config.staking_contract)?,
            staking_type: config.staking_type,
            max_ltv: config.max_ltv,
            performance_fee_rate: config.performance_fee_rate,
            liquidation_fee_rate: config.liquidation_fee_rate,
        },
    )?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("action", "callback: update_config")],
        data: None,
    })
}

//----------------------------------------------------------------------------------------
// QUERY FUNCTIONS
//----------------------------------------------------------------------------------------

/**
 * @notice Return strategy configurations.
 */
pub fn query_config<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<ConfigResponse> {
    let config = read_config(&deps.storage)?;
    let operators = config
        .operators
        .iter()
        .map(|operator_raw| deps.api.human_address(&operator_raw).unwrap())
        .collect();

    Ok(ConfigResponse {
        owner: deps.api.human_address(&config.owner)?,
        operators,
        treasury: deps.api.human_address(&config.treasury)?,
        asset_token: deps.api.human_address(&config.asset_token)?,
        reward_token: deps.api.human_address(&config.reward_token)?,
        pool: deps.api.human_address(&config.pool)?,
        pool_token: deps.api.human_address(&config.pool_token)?,
        mars: deps.api.human_address(&config.mars)?,
        staking_contract: deps.api.human_address(&config.staking_contract)?,
        staking_type: config.staking_type,
        max_ltv: config.max_ltv,
        performance_fee_rate: config.performance_fee_rate,
        liquidation_fee_rate: config.liquidation_fee_rate,
    })
}

/**
 * @notice Return the global state of the strategy.
 */
pub fn query_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<StateResponse> {
    let state = read_state(&deps.storage)?;
    let (bond_value, debt_value, ltv) = compute_ltv(deps, None)?;

    Ok(StateResponse {
        total_bond_value: bond_value,
        total_bond_units: state.total_bond_units,
        total_debt_value: debt_value,
        total_debt_units: state.total_debt_units,
        ltv,
    })
}

/**
 * @notice Return data on an individual user's position.
 */
pub fn query_position<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    user: HumanAddr,
) -> StdResult<PositionResponse> {
    let position = read_position(&deps.storage, &deps.api.canonical_address(&user)?)?;
    let (bond_value, debt_value, ltv) = compute_ltv(deps, Some(user))?;

    Ok(PositionResponse {
        is_active: position.is_active(),
        bond_value,
        bond_units: position.bond_units,
        debt_value,
        debt_units: position.debt_units,
        ltv,
        unbonded_ust_amount: position.unbonded_ust_amount,
        unbonded_asset_amount: position.unbonded_asset_amount,
    })
}
