use cosmwasm_std::{
    entry_point, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order,
    Response, StdError, StdResult, Uint128,
};
use cw20::Cw20ReceiveMsg;

use fields_of_mars::adapters::Asset;

use mirror_protocol::staking::{
    Cw20HookMsg, ExecuteMsg, QueryMsg, RewardInfoResponse, RewardInfoResponseItem,
};

use crate::msg::InstantiateMsg;
use crate::state::{Config, BOND_AMOUNT, CONFIG, STAKING_TOKEN};

static MOCK_REWARD_AMOUNT: u128 = 1000000;

// INIT

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let config = Config {
        mirror_token: deps.api.addr_validate(&msg.mirror_token)?,
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => execute_receive_cw20(deps, env, info, cw20_msg),
        ExecuteMsg::RegisterAsset {
            asset_token,
            staking_token,
        } => execute_register_asset(deps, env, info, asset_token, staking_token),
        ExecuteMsg::Unbond {
            asset_token,
            amount,
        } => execute_unbond(deps, env, info, asset_token, amount),
        ExecuteMsg::Withdraw { asset_token } => execute_withdraw(deps, env, info, asset_token),

        _ => Err(StdError::generic_err("unimplemented")),
    }
}

fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> StdResult<Response> {
    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::Bond { asset_token } => {
            let asset_token_addr = deps.api.addr_validate(&asset_token)?;
            let staking_token_addr = STAKING_TOKEN.load(deps.storage, &asset_token_addr)?;

            if info.sender != staking_token_addr {
                return Err(StdError::generic_err("unauthorized"));
            }

            execute_bond(
                deps,
                env,
                info,
                cw20_msg.sender,
                asset_token,
                cw20_msg.amount,
            )
        }

        _ => Err(StdError::generic_err("unimplemented")),
    }
}

fn execute_register_asset(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    asset_token: String,
    staking_token: String,
) -> StdResult<Response> {
    let asset_token_addr = deps.api.addr_validate(&asset_token)?;
    let staking_token_addr = deps.api.addr_validate(&staking_token)?;

    STAKING_TOKEN.save(deps.storage, &asset_token_addr, &staking_token_addr)?;

    Ok(Response::default())
}

fn execute_bond(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    staker: String,
    asset_token: String,
    amount: Uint128,
) -> StdResult<Response> {
    let staker_addr = deps.api.addr_validate(&staker)?;
    let asset_token_addr = deps.api.addr_validate(&asset_token)?;

    let bond_amount = helpers::load_bond_amount(deps.storage, &staker_addr, &asset_token_addr);

    BOND_AMOUNT.save(
        deps.storage,
        (&staker_addr, &asset_token_addr),
        &(bond_amount + amount),
    )?;

    Ok(Response::default())
}

fn execute_unbond(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset_token: String,
    amount: Uint128,
) -> StdResult<Response> {
    let asset_token_addr = deps.api.addr_validate(&asset_token)?;

    let bond_amount = helpers::load_bond_amount(deps.storage, &info.sender, &asset_token_addr);

    BOND_AMOUNT.save(
        deps.storage,
        (&info.sender, &asset_token_addr),
        &(bond_amount - amount),
    )?;

    let staking_token_addr = STAKING_TOKEN.load(deps.storage, &asset_token_addr)?;
    let outbound_asset = Asset::cw20(&staking_token_addr, amount);
    let outbound_msg = outbound_asset.transfer_msg(&info.sender)?;

    Ok(Response::new().add_message(outbound_msg))
}

fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    asset_token: Option<String>,
) -> StdResult<Response> {
    let reward_infos =
        helpers::read_reward_infos(deps.api, deps.storage, &info.sender, asset_token)?;
    let reward_amounts: Vec<Uint128> = reward_infos
        .iter()
        .map(|reward_info| reward_info.pending_reward)
        .collect();
    let total_reward_amount: Uint128 = reward_amounts.iter().sum();

    let config = CONFIG.load(deps.storage)?;
    let outbound_asset = Asset::cw20(&config.mirror_token, total_reward_amount);
    let outbound_msg = outbound_asset.transfer_msg(&info.sender)?;

    Ok(Response::new().add_message(outbound_msg))
}

/// QUERIES

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::RewardInfo {
            staker_addr,
            asset_token,
        } => to_binary(&query_reward_info(deps, env, staker_addr, asset_token)?),

        _ => Err(StdError::generic_err("unimplemented")),
    }
}

fn query_reward_info(
    deps: Deps,
    _env: Env,
    staker: String,
    asset_token: Option<String>,
) -> StdResult<RewardInfoResponse> {
    let staker_addr = deps.api.addr_validate(&staker)?;

    let reward_infos =
        helpers::read_reward_infos(deps.api, deps.storage, &staker_addr, asset_token)?;

    Ok(RewardInfoResponse {
        staker_addr: staker,
        reward_infos,
    })
}

mod helpers {
    use cosmwasm_std::{Api, Storage};

    use super::*;

    pub fn load_bond_amount(
        storage: &dyn Storage,
        staker_addr: &Addr,
        asset_token_addr: &Addr,
    ) -> Uint128 {
        BOND_AMOUNT
            .load(storage, (staker_addr, asset_token_addr))
            .unwrap_or_else(|_| Uint128::zero())
    }

    pub fn read_reward_infos(
        api: &dyn Api,
        storage: &dyn Storage,
        staker_addr: &Addr,
        asset_token: Option<String>,
    ) -> StdResult<Vec<RewardInfoResponseItem>> {
        let reward_infos = if let Some(asset_token) = asset_token {
            let asset_token_addr = api.addr_validate(&asset_token)?;
            let bond_amount = helpers::load_bond_amount(storage, staker_addr, &asset_token_addr);
            vec![RewardInfoResponseItem {
                asset_token,
                bond_amount,
                pending_reward: Uint128::new(MOCK_REWARD_AMOUNT),
                is_short: false,
            }]
        } else {
            STAKING_TOKEN
                .keys(storage, None, None, Order::Ascending)
                .map(|asset_token_bytes| {
                    let asset_token = String::from_utf8(asset_token_bytes).unwrap();
                    let asset_token_addr = api.addr_validate(&asset_token).unwrap();

                    let bond_amount =
                        helpers::load_bond_amount(storage, staker_addr, &asset_token_addr);

                    let pending_reward = if bond_amount.is_zero() {
                        Uint128::zero()
                    } else {
                        Uint128::new(MOCK_REWARD_AMOUNT)
                    };

                    RewardInfoResponseItem {
                        asset_token: asset_token_addr.into(),
                        bond_amount,
                        pending_reward,
                        is_short: false,
                    }
                })
                .collect()
        };

        Ok(reward_infos)
    }
}
