use cosmwasm_std::{
    attr, Addr, Attribute, CosmosMsg, Decimal, DepsMut, Env, Event, MessageInfo, Response,
    StdError, StdResult, Storage,
};

use cw_asset::{Asset, AssetInfo, AssetList};

use fields_of_mars::martian_field::{Config, Action, CallbackMsg};

use crate::health::compute_health;
use crate::helpers::assert_sent_fund;
use crate::state::{State, CONFIG, POSITION, STATE};

pub fn init_storage(deps: DepsMut, config: Config) -> StdResult<Response> {
    CONFIG.save(deps.storage, &config)?;
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
    let config = CONFIG.load(deps.storage)?;
    
    let mut received_coins = AssetList::from(info.funds);
    let mut msgs: Vec<CosmosMsg> = vec![];
    let mut attrs: Vec<Attribute> = vec![];
    let mut callbacks: Vec<CallbackMsg> = vec![];

    // compose a list of callback messages based on user-selected actions
    for action in actions {
        match action {
            Action::Deposit(asset) => handle_deposit(
                deps.storage,
                &env.contract.address,
                &info.sender,
                &mut received_coins,
                &asset.check(api, None)?,
                &mut msgs,
                &mut attrs,
            )?,
            Action::Borrow { amount } => callbacks.push(
                CallbackMsg::Borrow {
                    user_addr: info.sender.clone(),
                    borrow_amount: amount,
                }
            ),
            Action::Repay { amount } => callbacks.push(
                CallbackMsg::Repay {
                    user_addr: info.sender.clone(),
                    repay_amount: Some(amount),
                }
            ),
            Action::Bond { slippage_tolerance } => callbacks.extend([
                CallbackMsg::ProvideLiquidity {
                    user_addr: Some(info.sender.clone()),
                    slippage_tolerance,
                },
                CallbackMsg::Bond {
                    user_addr: Some(info.sender.clone()),
                },
            ]),
            Action::Unbond { bond_units_to_reduce } => callbacks.extend([
                CallbackMsg::Unbond {
                    user_addr: info.sender.clone(),
                    bond_units_to_reduce,
                },
                CallbackMsg::WithdrawLiquidity {
                    user_addr: info.sender.clone(),
                },
            ]),
            Action::Swap { offer_amount, max_spread } => callbacks.push(
                CallbackMsg::Swap {
                    user_addr: Some(info.sender.clone()),
                    offer_asset_info: config.primary_asset_info.clone(),
                    offer_amount: Some(offer_amount),
                    max_spread,
                }
            ),
        }
    }

    // after all deposits have been handled, we assert that the `received_natives` list is empty
    // this way, we ensure that the user does not send any extra fund which will get lost in the 
    // contract
    if received_coins.len() > 0 {
        return Err(StdError::generic_err(
            format!("extra funds received: {}", received_coins)
        ));
    }

    // after user selected actions, we executes two more callbacks:
    // - refund assets that are not deployed in the yield farm to user
    // - assert LTV is healthy; if not, throw error and revert all actions
    callbacks.extend([
        CallbackMsg::Refund {
            user_addr: info.sender.clone(),
            recipient_addr: info.sender.clone(),
            percentage: Decimal::one(),
        },
        CallbackMsg::AssertHealth {
            user_addr: info.sender.clone(),
        },
        CallbackMsg::PurgeStorage {
            user_addr: info.sender.clone(),
        },
    ]);

    let callback_msgs = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address))
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field/execute/update_position")
        .add_attributes(attrs))
}

fn handle_deposit(
    storage: &mut dyn Storage,
    contract_addr: &Addr,
    sender_addr: &Addr,
    received_coins: &mut AssetList,
    asset: &Asset,
    msgs: &mut Vec<CosmosMsg>,
    attrs: &mut Vec<Attribute>,
) -> StdResult<()> {
    // if deposit amount is zero, we do nothing
    if asset.amount.is_zero() {
        return Ok(());
    }

    // If asset is a native token, we assert that the same amount was indeed received
    // If asset is a CW20 token, we:
    // - Transfer the specified amount from the user's wallet
    // - Remove the asset from the list. After every deposit action has been processed, assert that 
    // the asset list is empty. This way, we ensure the user doesn't send any extra fund, which will 
    // be lost in the contract
    match &asset.info {
        AssetInfo::Cw20(_) => {
            msgs.push(asset.transfer_from_msg(sender_addr, contract_addr)?);
        }
        AssetInfo::Native(_) => {
            assert_sent_fund(asset, received_coins)?;
            received_coins.deduct(asset)?;
        }
    }

    // increase the user's unlocked asset amount
    let mut position = POSITION.load(storage, sender_addr).unwrap_or_default();
    position.unlocked_assets.add(asset)?;
    POSITION.save(storage, sender_addr, &position)?;

    attrs.push(attr("deposit_received", asset.to_string()));

    Ok(())
}

pub fn harvest(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    max_spread: Option<Decimal>,
    slippage_tolerance: Option<Decimal>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // only whitelisted operators can harvest
    if !config.operators.contains(&info.sender) {
        return Err(StdError::generic_err("caller is not a whitelisted operator"));
    }

    // find how much reward is available to be claimed
    let rewards = config.astro_generator.query_rewards(
        &deps.querier,
        &env.contract.address,
        &config.primary_pair.liquidity_token,
    )?;

    // if reward amount is non-zero, we construct a message to claim them, as well as add them to
    // the pending rewards
    let mut msgs: Vec<CosmosMsg> = vec![];
    if rewards.len() > 0 {
        msgs.push(
            config.astro_generator.claim_rewards_msg(&config.primary_pair.liquidity_token)?
        );
        state.pending_rewards.add_many(&rewards)?;
    }

    // a portion of the pending rewards will be charged as fees
    let mut fees = state.pending_rewards.clone();
    fees.apply(|asset| asset.amount = asset.amount * config.fee_rate);
    fees.purge();
    msgs.extend(fees.transfer_msgs(&config.treasury)?);

    // deduct fees from available rewards. the remaining amounts are to be reinvested
    state.pending_rewards.deduct_many(&fees)?;
    STATE.save(deps.storage, &state)?;

    // if there are ASTRO tokens available to be reinvested, we first swap it to the secondary asset
    // asset
    let mut callbacks: Vec<CallbackMsg> = vec![];
    if let Some(astro_token) = state.pending_rewards.find(&config.astro_token_info) {
        callbacks.push(CallbackMsg::Swap {
            user_addr: None,
            offer_asset_info: config.astro_token_info.clone(),
            offer_amount: Some(astro_token.amount),
            max_spread,
        });
    }

    // once ASTRO is sold, pending rewards should only consist of primary and secondary assets
    // 1. doing a swap so that their values are balanced
    // 2. provide liquidity
    // 3. bond liquidity tokens (without increasing total bond units)
    callbacks.extend([
        CallbackMsg::Balance {
            max_spread,
        },
        CallbackMsg::ProvideLiquidity {
            user_addr: None,
            slippage_tolerance,
        },
        CallbackMsg::Bond {
            user_addr: None,
        },
    ]);

    let callback_msgs = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address))
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    let event = Event::new("harvested")
        .add_attribute("time", env.block.time.seconds().to_string())
        .add_attribute("height", env.block.height.to_string())
        .add_attribute("fees", fees.to_string());

    Ok(Response::new()
        .add_messages(msgs)
        .add_messages(callback_msgs)
        .add_attribute("action", "martian_field/execute/harvest")
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

    // if `health.ltv` is `Some`, it must be greater than `max_ltv`
    // if `health.ltv` is `None`, indicating the position is already closed, then it is not liquidatable
    let ltv = health.ltv.ok_or_else(|| StdError::generic_err("position is already closed"))?;
    if ltv <= config.max_ltv {
        return Err(StdError::generic_err("position is healthy"));
    }

    // 1. unbond the user's liquidity tokens from Astro generator
    // 2. burn liquidity tokens, withdraw primary + secondary assets from the pool
    // 3. swap all primary assets to secondary assets
    // 4. repay all debts
    // 5. among all remaining assets, send the amount corresponding to `bonus_rate` to the liquidator
    // 6. refund all assets that're left to the user
    //
    // NOTE: in the previous versions, we sell **all** primary assets, which is not optimal because 
    // this will incur bigger slippage, causing worse liquidation cascade, and be potentially lucrative 
    // for sandwich attackers
    //
    // now, we calculate how much additional secondary asset is needed to fully pay off debt, and 
    // reverse-simulate how much primary asset needs to be sold
    let callbacks = [
        CallbackMsg::Unbond {
            user_addr: user_addr.clone(),
            bond_units_to_reduce: position.bond_units,
        },
        CallbackMsg::WithdrawLiquidity {
            user_addr: user_addr.clone(),
        },
        CallbackMsg::Cover {
            user_addr: user_addr.clone(),
        },
        CallbackMsg::Repay {
            user_addr: user_addr.clone(),
            repay_amount: None,
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
        CallbackMsg::ClearBadDebt {
            user_addr: user_addr.clone(),
        },
        CallbackMsg::PurgeStorage {
            user_addr: user_addr.clone(),
        },
    ];

    let callback_msgs = callbacks
        .iter()
        .map(|callback| callback.into_cosmos_msg(&env.contract.address))
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

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
        .add_attribute("action", "martian_field/execute/liquidate")
        .add_event(event))
}

pub fn update_config(deps: DepsMut, info: MessageInfo, new_config: Config) -> StdResult<Response> {
    // Only governance can update config
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.governance {
        return Err(StdError::generic_err("only governance can update config"));
    }

    // New config must be valid
    config.validate()?;

    CONFIG.save(deps.storage, &new_config)?;
    Ok(Response::default())
}
