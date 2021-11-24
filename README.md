# Martian Field

Martian Field is a leveraged yield farming strategy utilizing contract-to-contract (C2C) lending from [Mars Protocol](https://twitter.com/mars_protocol).

## Overview

A common type of yield farms in the Terra ecosystem works as follows. The user provides a _primary asset_ (e.g. ANC, Anchor Protocol's governance token) and equal value of UST to an AMM pool (e.g. Terraswap), then deposit the AMM's liquidity token into a staking contract. Over time, staking reward is accrued in the form of the primary asset (ANC in this case) and withdrawable by the user.

To reinvest the farming gains, the user needs to

1. claim staking reward
2. sell half of the reward to UST
3. provide the UST and the other half of the reward to the AMM
4. deposit the liquidity token to the staking contract

**Martian Field** is an autocompounder that 1) automates this process, and 2) allow user to take up to 2x leverage utilizing C2C lending from Mars protocol.

Martian Field also tracks each user's loan-to-value ratio (LTV). If a user's LTV exceeds a preset threshold, typically as a result of the primary asset's price falling or debt builds up too quickly, the position is subject to liquidation.

## Development

### Dependencies

- Rust v1.44.1+
- `wasm32-unknown-unknown` target
- Docker
- [LocalTerra](https://github.com/terra-project/LocalTerra)
- Node.js v16

### Envrionment Setup

1. Install `rustup` via https://rustup.rs/

2. Add `wasm32-unknown-unknown` target

```sh
rustup default stable
rustup target add wasm32-unknown-unknown
```

3. Install [Docker](https://www.docker.com/)

4. Clone the [LocalTerra](https://github.com/terra-project/LocalTerra#usage) repository, edit `config/genesis.json` as follows. This fixes the rate of stability fee (aka "tax") charged on UST transfers to the value of 0.1%, which gives us deterministic and preditable results.

```diff
"app_state": {
  "treasury": {
    "params": {
      "tax_policy": {
-       "rate_min": "0.000500000000000000",
-       "rate_max": "0.010000000000000000",
+       "rate_min": "0.001000000000000000",
+       "rate_max": "0.001000000000000000",
      },
-     "change_rate_max": "0.000250000000000000"
+     "change_rate_max": "0.000000000000000000"
    }
  }
}
```

5. Optionally, [speed up LocalTerra's blocktime](https://github.com/terra-project/LocalTerra#pro-tip-speed-up-block-time) by changing `config/config.toml` as follows:

```diff
##### consensus configuration options #####
[consensus]

wal_file = "data/cs.wal/wal"
- timeout_propose = "3s"
- timeout_propose_delta = "500ms"
- timeout_prevote = "1s"
- timeout_prevote_delta = "500ms"
- timeout_precommit_delta = "500ms"
- timeout_commit = "5s"
+ timeout_propose = "200ms"
+ timeout_propose_delta = "200ms"
+ timeout_prevote = "200ms"
+ timeout_prevote_delta = "200ms"
+ timeout_precommit_delta = "200ms"
+ timeout_commit = "200ms"
```

6. Install Node, preferrably using [nvm](https://github.com/nvm-sh/nvm#installing-and-updating), as well as libraries required for testing:

```bash
nvm install 16
nvm alias default 16
cd fields-of-mars/scripts
npm install
```

### Compile

Make sure the current working directory is set to the root directory of this repository, then

```bash
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.11.4
```

### Test

Start LocalTerra:

```bash
cd /path/to/LocalTerra
git checkout main
git pull
docker-compose up
```

Run test scripts: inside `scripts` folder,

```bash
ts-node 1_cw20_token.spec.ts
ts-node 2_astroport.spec.ts
ts-node 3_mock_red_bank.spec.ts
ts-node 4_mock_oracle.spec.ts
ts-node 5_mock_anchor.spec.ts
ts-node 6_mock_mirror.spec.ts
ts-node 7_martian_field.spec.ts
```

### Deploy

Provide seed phrases in `scripts/.env` file, then:

```bash
ts-node deploy.ts --network {columbus|bombay} --strategy {anchor|mirror|mars} [--code-id <codeId>]
```

### Notes

- LocalTerra [only works on X86 processors](https://github.com/terra-project/LocalTerra#requirements). There is currently no way to run the tests on Macs with the M1 processor.

- VS Code users are recommended to install `rust-lang.rust` and `esbenp.prettier-vscode` plugins, and open the workspace from `field-of-mars.code-workspace` included in the base directory of this repo, which contains some helpful configurations.

## Deployment

### Mainnet

| Contract                           | Address                                                                                                                                      |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Anchor Token                       | [`terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76`](https://finder.terra.money/columbus-5/address/terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76) |
| Anchor Staking                     | [`terra1897an2xux840p9lrh6py3ryankc6mspw49xse3`](https://finder.terra.money/columbus-5/address/terra1897an2xux840p9lrh6py3ryankc6mspw49xse3) |
| Astroport ANC-UST Pair             | TBD                                                                                                                                          |
| Astroport ANC-UST liquidity Token  | TBD                                                                                                                                          |
| Mirror Token                       | [`terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6`](https://finder.terra.money/columbus-5/address/terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6) |
| Mirror Staking                     | [`terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5`](https://finder.terra.money/columbus-5/address/terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5) |
| Astroport MIR-UST Pair             | TBD                                                                                                                                          |
| Astroport MIR-UST liquidity Token  | TBD                                                                                                                                          |
| Pylon Token                        | [`terra1kcthelkax4j9x8d3ny6sdag0qmxxynl3qtcrpy`](https://finder.terra.money/columbus-5/address/terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6) |
| Pylon Staking                      | [`terra19nek85kaqrvzlxygw20jhy08h3ryjf5kg4ep3l`](https://finder.terra.money/columbus-5/address/terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5) |
| Astroport MINE-UST Pair            | TBD                                                                                                                                          |
| Astroport MINE-UST liquidity Token | TBD                                                                                                                                          |
| Mars Red Bank                      | TBD                                                                                                                                          |
| Mars Oracle                        | TBD                                                                                                                                          |
| Martian Field: ANC-UST             | TBD                                                                                                                                          |
| Martian Field: MIR-UST             | TBD                                                                                                                                          |
| Martian Field: MINE-UST            | TBD                                                                                                                                          |

### Testnet

| Contract                           | Address                                                                                                                                     |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Anchor Token                       | [`terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc`](https://finder.terra.money/bombay-12/address/terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc) |
| Anchor Staking                     | [`terra19nxz35c8f7t3ghdxrxherym20tux8eccar0c3k`](https://finder.terra.money/bombay-12/address/terra19nxz35c8f7t3ghdxrxherym20tux8eccar0c3k) |
| Astroport ANC-UST Pair             | TBD                                                                                                                                         |
| Astroport ANC-UST liquidity Token  | TBD                                                                                                                                         |
| Mirror Token                       | [`terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u`](https://finder.terra.money/bombay-12/address/terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u) |
| Mirror Staking                     | [`terra1a06dgl27rhujjphsn4drl242ufws267qxypptx`](https://finder.terra.money/bombay-12/address/terra1a06dgl27rhujjphsn4drl242ufws267qxypptx) |
| Astroport MIR-UST Pair             | TBD                                                                                                                                         |
| Astroport MIR-UST liquidity Token  | TBD                                                                                                                                         |
| Pylon Token                        | [`terra1lqm5tutr5xcw9d5vc4457exa3ghd4sr9mzwdex`](https://finder.terra.money/bombay-12/address/terra1lqm5tutr5xcw9d5vc4457exa3ghd4sr9mzwdex) |
| Pylon Staking                      | [`terra17av0lfhqymusm6j9jpepzerg6u54q57jp7xnrz`](https://finder.terra.money/bombay-12/address/terra17av0lfhqymusm6j9jpepzerg6u54q57jp7xnrz) |
| Astroport MINE-UST Pair            | TBD                                                                                                                                         |
| Astroport MINE-UST liquidity Token | TBD                                                                                                                                         |
| Mars Red Bank                      | TBD                                                                                                                                         |
| Mars Oracle                        | TBD                                                                                                                                         |
| Martian Field: ANC-UST             | TBD                                                                                                                                         |
| Martian Field: MIR-UST             | TBD                                                                                                                                         |
| Martian Field: MINE-UST            | TBD                                                                                                                                         |

## License

Contents of this repository are open source under [GNU General Public License v3](https://www.gnu.org/licenses/gpl-3.0.en.html).
