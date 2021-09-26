# Fields of Mars: Common Types

This crate contains common types used in Martian Field, as well as **adapters** used to communicate with other contracts.

## Adapters

Adapters provide a modular way to execute or query other contracts. This crates includes the following adapters:

| adapter             | description                          |
| ------------------- | ------------------------------------ |
| `adapters::Asset`   | Terra stablecoins or CW20 tokens     |
| `adapters::Oracle`  | Mars Protocol oracle                 |
| `adapters::Pair`    | Astroport pairs                      |
| `adapters::RedBank` | Mars Protocol money market           |
| `adapters::Staking` | Anchor or Mirror V2 staking contract |

#### Example 1. Sending UST

```rust
let recipient_addr = Addr::unchecked("recipient");
let denom = "uusd";
let amount = 123456;
```

Without using adapter:

```rust
use cosmwasm_std::{BankMsg, CosmosMsg, Response};

let msg = CosmosMsg::Bank(BankMsg::Send {
    to_address: recipient.to_string(),
    amount: vec![Coin {
        denom: denom.to_string(),
        amount: Uint128::new(amount),
    }],
});
```

Using `Asset` adapter:

```rust
use fields_of_mars::adapters::Asset;

let msg = Asset::native(denom, amount).transfer_msg(&recipient)?;
```

#### Example 2. Provide liquidity to Astroport ANC-UST pool

```rust
let anchor_token_addr = Addr::unchecked("anchor_token");
let pair_addr = Addr::unchecked("astroport_pair");
let share_token_addr = Add::unchecked("astroport_share_token");
let amount_ust = 88888;
let amount_anc = 69420;
```

Without using adapter:

```rust
use cosmwasm_std::{Coin, CosmosMsg, WasmMsg};
use cw20::{ExecuteMsg as Cw20ExecuteMsg};
use astroport;

let assets = [
    Asset {
        info: AssetInfo::NativeToken {
            denom: "uusd".to_string()
        },
        amount: Uint128::new(amount_ust)
    },
    Asset {
        info: AssetInfo::Token {
            contract_addr: anchor_token_addr
        },
        amount: Uint128::new(amount_anc)
    }
];

let msgs = vec![
    CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: anchor_token_addr.to_string(),
        msg: to_binary(&Cw20ExecuteMsg::IncreaseAllowance {
            spender: pair_addr.to_string(),
            amount: Uint128::new(amount_anc),
            expires: None,
        })?,
        funds: vec![],
    }),
    CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: pair_addr.to_string(),
        msg: to_binary(&astroport::ExecuteMsg::ProvideLiquidity {
            assets,
            slippage_tolerance: None,
        })?,
        funds: vec![Coin {
            denom: "uusd".to_string(),
            amount: Uint128::new(amount_ust)
        }],
    })
];
```

Using adapter:

```rust
use fields_of_mars::adapters::{Asset, Pair};

let assets = [
    Asset::native("uusd", amount_ust),
    Asset::cw20(anchor_token_addr, amount_anc)
];

let msgs = Pair::new(pair_addr, share_token_addr).provide_msgs(&assets)?;
```
