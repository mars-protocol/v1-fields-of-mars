use cosmwasm_std::{
    attr, Addr, Attribute, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo, Response,
    StdError, StdResult, Storage,
};

use fields_of_mars::adapters::Asset;
use fields_of_mars::martian_field::msg::{Action, CallbackMsg};
use fields_of_mars::martian_field::{Config, ConfigUnchecked, State};

use crate::helpers::*;
use crate::state::{CONFIG, POSITION, STATE};

pub fn init_storage(deps: DepsMut, msg: ConfigUnchecked) -> StdResult<Response> {
    CONFIG.save(deps.storage, &msg.check(deps.api)?)?;
    STATE.save(deps.storage, &State::default())?;
    Ok(Response::default())
}

pub fn update_position(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    actions: Vec<Action>,
) -> StdResult<Response> {
    let api = deps.api;

    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attrs: Vec<Attribute> = vec![];
    let mut callbacks: Vec<CallbackMsg> = vec![];

    // compose a list of callback messages based on user-selected actions
    for action in actions {
        match action {
            Action::Deposit(asset) => handle_deposit(
                deps.storage,
                &env,
                &info,
                &asset.check(api)?,
                &mut msgs,
                &mut attrs,
            )?,

            Action::Borrow {
                amount,
            } => callbacks.push(CallbackMsg::Borrow {
                user_addr: info.sender.clone(),
                borrow_amount: amount,
            }),

            Action::Repay {
                amount,
            } => callbacks.push(CallbackMsg::Repay {
                user_addr: info.sender.clone(),
                repay_amount: amount,
            }),

            Action::Bond {
                slippage_tolerance,
            } => callbacks.extend([
                CallbackMsg::ProvideLiquidity {
                    user_addr: info.sender.clone(),
                    slippage_tolerance,
                },
                CallbackMsg::Bond {
                    user_addr: info.sender.clone(),
                },
            ]),

            Action::Unbond {
                bond_units_to_reduce,
            } => callbacks.extend([
                CallbackMsg::Unbond {
                    user_addr: info.sender.clone(),
                    bond_units_to_reduce,
                },
                CallbackMsg::WithdrawLiquidity {
                    user_addr: info.sender.clone(),
                },
            ]),

            Action::Swap {
                swap_amount,
                belief_price,
                max_spread,
            } => callbacks.push(CallbackMsg::Swap {
                user_addr: info.sender.clone(),
                swap_amount: Some(swap_amount),
                belief_price,
                max_spread,
            }),
        }
    }

    // after user selected actions, we executes three more callbacks:
    //
    // - refund assets that are not deployed in the yield farm to user
    //
    // - assert LTV is healthy; if not, throw error and revert all actions
    //
    // - save a snapshot of a user's position. this is only needed for the frontend to calculate
    // user's PnE. this can be removed once we have the infra ready to calculate this off-chain
    callbacks.extend([
        CallbackMsg::Refund {
            user_addr: info.sender.clone(),
            recipient_addr: info.sender.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::AssertHealth {
            user_addr: info.sender.clone(),
        },
        // TODO: remove this
        CallbackMsg::Snapshot {
            user_addr: info.sender.clone(),
        },
    ]);

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: excute :: update_position"))
}

fn handle_deposit(
    storage: &mut dyn Storage,
    env: &Env,
    info: &MessageInfo,
    asset: &Asset,
    msgs: &mut Vec<CosmosMsg>,
    attrs: &mut Vec<Attribute>,
) -> StdResult<()> {
    // if deposit amount is zero, we do nothing
    if asset.amount.is_zero() {
        return Ok(());
    }

    // if asset is a native token, we assert that the same amount was indeed received
    if asset.info.is_native() {
        asset.assert_sent_amount(&info.funds)?;
    }
    // if asset is a CW20 token, we transfer the specified amount from the user's wallet
    //
    // NOTE: user must have approved spending limit
    else {
        msgs.push(asset.transfer_from_msg(&info.sender, &env.contract.address)?);
    }

    // increase the user's unlocked asset amount
    let mut position = POSITION.load(storage, &info.sender).unwrap_or_default();
    add_unlocked_asset(&mut position, &asset);
    POSITION.save(storage, &info.sender, &position)?;

    attrs.push(attr("deposit_received", asset.to_string()));

    Ok(())
}

pub fn harvest(
    deps: DepsMut,
    env: Env,
    belief_price: Option<Decimal>,
    max_spread: Option<Decimal>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<Response> {
    // We use a fake address to track assets handled during harvesting
    let reward_addr = Addr::unchecked("reward");

    let config = CONFIG.load(deps.storage)?;
    let mut position = POSITION.load(deps.storage, &reward_addr).unwrap_or_default();

    // Find how much reward is available to be claimed
    let (_, reward_amount) =
        config.staking.query_reward_info(&deps.querier, &env.contract.address, env.block.height)?;

    // If there is no reward to claim, then we do nothing
    if reward_amount.is_zero() {
        return Ok(Response::default());
    }

    // We assume the reward is the primary asset
    // Among the claimable reward, a portion corresponding to `config.fee_rate` is charged as fee
    // and sent to the treasury address. Among the rest, half is to be swapped for the secondary asset
    let fee_amount = reward_amount * config.fee_rate;
    let retain_amount = reward_amount - fee_amount;
    let swap_amount = retain_amount.multiply_ratio(1u128, 2u128);

    let primary_asset_as_fee = Asset::new(&config.primary_asset_info, fee_amount);
    let primary_asset_to_retain = Asset::new(&config.primary_asset_info, retain_amount);
    let primary_asset_to_swap = Asset::new(&config.primary_asset_info, swap_amount);

    add_unlocked_asset(&mut position, &primary_asset_to_retain);
    POSITION.save(deps.storage, &reward_addr, &position)?;

    let msgs = vec![
        config.staking.withdraw_msg()?,
        primary_asset_as_fee.deduct_tax(&deps.querier)?.transfer_msg(&config.treasury)?,
    ];

    let callbacks = [
        CallbackMsg::Swap {
            user_addr: reward_addr.clone(),
            swap_amount: Some(primary_asset_to_swap.deduct_tax(&deps.querier)?.amount),
            belief_price,
            max_spread,
        },
        CallbackMsg::ProvideLiquidity {
            user_addr: reward_addr.clone(),
            slippage_tolerance,
        },
        CallbackMsg::Bond {
            user_addr: reward_addr,
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    let event = Event::new("harvested")
        .add_attribute("time", env.block.time.seconds().to_string())
        .add_attribute("height", env.block.height.to_string())
        .add_attribute("fee_amount", fee_amount)
        .add_attribute("retain_amount", retain_amount);

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: execute :: harvest")
        .add_event(event))
}

pub fn liquidate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    user_addr: Addr,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let position = POSITION.load(deps.storage, &user_addr).unwrap_or_default();

    // position must be active (LTV is not `None`) and the LTV must be greater than `max_ltv`
    let health = compute_health(&deps.querier, &env, &config, &state, &position)?;

    let ltv = health.ltv.ok_or_else(|| StdError::generic_err("position is already closed"))?;

    if ltv <= config.max_ltv {
        return Err(StdError::generic_err("position is healthy"));
    }

    let callbacks = [
        CallbackMsg::Unbond {
            user_addr: user_addr.clone(),
            bond_units_to_reduce: position.bond_units,
        },
        CallbackMsg::WithdrawLiquidity {
            user_addr: user_addr.clone(),
        },
        CallbackMsg::Swap {
            user_addr: user_addr.clone(),
            swap_amount: None,
            belief_price: None,
            max_spread: None,
        },
        CallbackMsg::Repay {
            user_addr: user_addr.clone(),
            repay_amount: health.debt_value,
        },
        CallbackMsg::Refund {
            user_addr: user_addr.clone(),
            recipient_addr: info.sender.clone(),
            percentage: config.bonus_rate,
        },
        CallbackMsg::Refund {
            user_addr: user_addr.clone(),
            recipient_addr: user_addr.clone(),
            percentage: Decimal::one(),
        },
    ];

    let callback_msgs: Vec<CosmosMsg> = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address).unwrap())
        .collect();

    let event = Event::new("liquidated")
        .add_attribute("liquidator", info.sender)
        .add_attribute("user", user_addr)
        .add_attribute("bond_units", position.bond_units)
        .add_attribute("debt_units", position.debt_units)
        .add_attribute("bond_value", health.bond_value)
        .add_attribute("debt_value", health.debt_value)
        .add_attribute("ltv", ltv.to_string());

    Ok(Response::new()
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field :: excute :: liquidate")
        .add_event(event))
}

pub fn update_config(deps: DepsMut, info: MessageInfo, new_config: Config) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    if info.sender != config.governance {
        return Err(StdError::generic_err("only governance can update config"));
    }

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::default())
}
