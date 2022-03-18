use cosmwasm_std::testing::{
    mock_dependencies, mock_env, mock_info, MockApi, MockQuerier, MockStorage,
};
use cosmwasm_std::{Addr, Coin, Decimal, OwnedDeps, StdError};

use cw_asset::{Asset, AssetInfo};

use fields_of_mars::adapters::{Generator, Oracle, Pair, RedBank};
use fields_of_mars::martian_field::{Action, ExecuteMsg};
use fields_of_mars::martian_field::Config;

use crate::contract::{execute, instantiate};

/// Deploy the contract, returns the `deps` object
fn setup_test() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
    let mut deps = mock_dependencies(&[]);

    let config = Config {
        primary_asset_info: AssetInfo::native("uluna"),
        secondary_asset_info: AssetInfo::native("uusd"),
        astro_token_info: AssetInfo::cw20(Addr::unchecked("astro_token")),
        primary_pair: Pair {
            contract_addr: Addr::unchecked("uluna_uusd_pair"),
            liquidity_token: Addr::unchecked("uluna_uusd_lp_token"),
        },
        astro_pair: Pair {
            contract_addr: Addr::unchecked("astro_uusd_pair"),
            liquidity_token: Addr::unchecked("astro_uusd_lp_token"),
        },
        astro_generator: Generator {
            contract_addr: Addr::unchecked("astro_generator"),
        },
        red_bank: RedBank {
            contract_addr: Addr::unchecked("red_bank"),
        },
        oracle: Oracle {
            contract_addr: Addr::unchecked("oracle"),
        },
        treasury: Addr::unchecked("treasury"),
        governance: Addr::unchecked("governance"),
        operators: vec![Addr::unchecked("operator")],
        max_ltv: Decimal::from_ratio(65u128, 100u128),
        fee_rate: Decimal::from_ratio(5u128, 100u128),
        bonus_rate: Decimal::from_ratio(1u128, 100u128),
    };

    instantiate(deps.as_mut(), mock_env(), mock_info("deployer", &[]), config.into()).unwrap();

    deps
}

#[test]
fn handling_native_deposits() {
    let mut deps = setup_test();

    // missing fund
    let deposits = vec![Coin::new(12345, "uluna"), ];
    let msg = ExecuteMsg::UpdatePosition(vec![
        Action::Deposit(Asset::native("uluna", 12345u128).into()),
        Action::Deposit(Asset::native("uusd", 67890u128).into()),
    ]);
    let res = execute(deps.as_mut(), mock_env(), mock_info("alice", &deposits), msg);
    assert_eq!(res, Err(StdError::generic_err("sent fund mismatch! expected: native:uusd:67890, received 0")));

    // fund amount mismatch
    let deposits = vec![
        Coin::new(12345, "uluna"), 
        Coin::new(69420, "uusd"), 
    ];
    let msg = ExecuteMsg::UpdatePosition(vec![
        Action::Deposit(Asset::native("uluna", 12345u128).into()),
        Action::Deposit(Asset::native("uusd", 67890u128).into()),
    ]);
    let res = execute(deps.as_mut(), mock_env(), mock_info("alice", &deposits), msg);
    assert_eq!(res, Err(StdError::generic_err("sent fund mismatch! expected: native:uusd:67890, received 69420")));

    // extra fund
    let deposits = vec![
        Coin::new(12345, "uluna"), 
        Coin::new(69420, "uusd"), 
        Coin::new(88888, "uatom"),
    ];
    let msg = ExecuteMsg::UpdatePosition(vec![
        Action::Deposit(Asset::native("uluna", 12345u128).into()),
        Action::Deposit(Asset::native("uusd", 69420u128).into()),
    ]);
    let res = execute(deps.as_mut(), mock_env(), mock_info("alice", &deposits), msg);
    assert_eq!(res, Err(StdError::generic_err("extra funds received: native:uatom:88888")));
}
