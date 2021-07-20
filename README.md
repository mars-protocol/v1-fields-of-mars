# Martian Field

**Martian Field** is a leveraged yield farming strategy utilizing liquidity from [Mars Protocol](https://twitter.com/mars_protocol).

## Overview

Users may deposit one of the following assets:

- ANC ([@anchor_protocol](https://twitter.com/anchor_protocol))
- MIR ([@mirror_protocol](https://twitter.com/mirror_protocol))
- MINE ([@pylon_protocol](https://twitter.com/pylon_protocol))
- MARS ([@mars_protocol](https://twitter.com/mars_protocol))
- ASTRO ([@astroport_fi](https://twitter.com/astroport_fi))

The strategy borrows UST as uncollateralized loans from Mars, and provides the assets to an AMM pool (TerraSwap or Astroport). The acquired share tokens are then bonded to the respective protocol's staking contract. Staking rewards are claimed and reinvested on a regular basis.

The strategy also tracks each user's loan-to-value ratio (LTV). If a user's LTV exceeds a preset threshold, typically as a result of the asset's price falling or debt builds up too quickly, the position is subject to liquidation.

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

### Compilation

Make sure the current working directory is set to the root directory of this repository, then

```bash
docker run --rm -v "$(pwd)":/code \
  --mount type=volume,source="$(basename "$(pwd)")_cache",target=/code/target \
  --mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
  cosmwasm/workspace-optimizer:0.11.4
```

### Tests

Start LocalTerra:

```bash
cd /path/to/LocalTerra
git checkout v0.4.1
docker-compose up
```

Run test scripts: inside `scripts` folder,

```bash
ts-node 1_terraswap_token.spec.ts
ts-node 2_terraswap_pair.spec.ts
ts-node 3_mock_mars.spec.ts
ts-node 4_mock_anchor.spec.ts
ts-node 5_mock_mirror.spec.ts
ts-node 6_martian_field.spec.ts
```

### Deployment

Provide seed phrases in `scripts/.env` file, then:

```bash
ts-node deploy.ts --network {columbus|tequila} --strategy {anchor|mirror} [--code-id <codeId>]
```

### Notes

- LocalTerra [only works on X86 processors](https://github.com/terra-project/LocalTerra#requirements). There is currently no way to run the tests on Macs with the M1 processor.

- VS Code users are recommended to install `rust-lang.rust` and `esbenp.prettier-vscode` plugins, and open the workspace from `field-of-mars.code-workspace` included in the base directory of this repo, which contains some helpful configurations.

## License

TBD
