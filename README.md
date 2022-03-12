# Martian Field

Martian Field is a leveraged yield farming strategy utilizing contract-to-contract (C2C) lending from [Mars Protocol](https://twitter.com/mars_protocol).

## Bug bounty

A bug bounty is currently open for these contracts. See details at: https://immunefi.com/bounty/marsprotocol/

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

4. Clone the [LocalTerra](https://github.com/terra-project/LocalTerra#usage) repository, edit `config/genesis.json` as follows. Set the stability fee ("tax") to zero by:

```diff
"app_state": {
  "treasury": {
    "params": {
      "tax_policy": {
-       "rate_min": "0.000500000000000000",
-       "rate_max": "0.010000000000000000",
+       "rate_min": "0.000000000000000000",
+       "rate_max": "0.000000000000000000",
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
  cosmwasm/workspace-optimizer:0.11.5
```

### Test

Start LocalTerra:

```bash
cd /path/to/LocalTerra
git checkout main
git pull
docker compose up
```

Run test scripts: inside `scripts` folder,

```bash
ts-node tests/1_mock_astro_generator.test.ts
ts-node tests/2_mock_oracle.test.ts
ts-node tests/3_mock_red_bank.test.ts
ts-node tests/4_martian_field.test.ts
```

### Deploy

Provide seed phrase of the deployer account in `scripts/.env`; create an `instantiate_msg.json` storing the contract's instantiate message; then

```bash
ts-node 1_deploy.ts --network mainnet|testnet --msg /path/to/instantiate_msg.json [--code-id codeId]
```

### Notes

- LocalTerra [only works on X86 processors](https://github.com/terra-project/LocalTerra#requirements). There is currently no way to run the tests on Macs with the M1 processor.

- Our development setup includes the VSCode text editor, [rust-analyzer](https://github.com/rust-analyzer/rust-analyzer) for Rust, and ESLint + Prettier for TypeScript.

## Deployment

### Mainnet

| Contract               | Address                                                                                                                                              |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| LUNA-UST Pair          | [`terra1m6ywlgn6wrjuagcmmezzz2a029gtldhey5k552`](https://finder.extraterrestrial.money/mainnet/address/terra1m6ywlgn6wrjuagcmmezzz2a029gtldhey5k552) |
| LUNA-UST LP Token      | [`terra1m24f7k4g66gnh9f7uncp32p722v0kyt3q4l3u5`](https://finder.extraterrestrial.money/mainnet/address/terra1m24f7k4g66gnh9f7uncp32p722v0kyt3q4l3u5) |
| ANC Token              | [`terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76`](https://finder.extraterrestrial.money/mainnet/address/terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76) |
| ANC-UST Pair           | [`terra1qr2k6yjjd5p2kaewqvg93ag74k6gyjr7re37fs`](https://finder.extraterrestrial.money/mainnet/address/terra1qr2k6yjjd5p2kaewqvg93ag74k6gyjr7re37fs) |
| ANC-UST LP Token       | [`terra1wmaty65yt7mjw6fjfymkd9zsm6atsq82d9arcd`](https://finder.extraterrestrial.money/mainnet/address/terra1wmaty65yt7mjw6fjfymkd9zsm6atsq82d9arcd) |
| MIR Token              | [`terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6`](https://finder.extraterrestrial.money/mainnet/address/terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6) |
| MIR-UST Pair           | [`terra143xxfw5xf62d5m32k3t4eu9s82ccw80lcprzl9`](https://finder.extraterrestrial.money/mainnet/address/terra143xxfw5xf62d5m32k3t4eu9s82ccw80lcprzl9) |
| MIR-UST LP Token       | [`terra17trxzqjetl0q6xxep0s2w743dhw2cay0x47puc`](https://finder.extraterrestrial.money/mainnet/address/terra17trxzqjetl0q6xxep0s2w743dhw2cay0x47puc) |
| ASTRO Token            | [`terra1xj49zyqrwpv5k928jwfpfy2ha668nwdgkwlrg3`](https://finder.extraterrestrial.money/mainnet/address/terra1xj49zyqrwpv5k928jwfpfy2ha668nwdgkwlrg3) |
| ASTRO-UST Pair         | [`terra1l7xu2rl3c7qmtx3r5sd2tz25glf6jh8ul7aag7`](https://finder.extraterrestrial.money/mainnet/address/terra1l7xu2rl3c7qmtx3r5sd2tz25glf6jh8ul7aag7) |
| ASTRO-UST LP Token     | [`terra17n5sunn88hpy965mzvt3079fqx3rttnplg779g`](https://finder.extraterrestrial.money/mainnet/address/terra17n5sunn88hpy965mzvt3079fqx3rttnplg779g) |
| Astro Generator        | [`terra1zgrx9jjqrfye8swykfgmd6hpde60j0nszzupp9`](https://finder.extraterrestrial.money/mainnet/address/terra1zgrx9jjqrfye8swykfgmd6hpde60j0nszzupp9) |
| Mars Oracle            | [`terra155awqc2dxvswu2sgqxlsvwm06xl3g9fw6d5vp7`](https://finder.extraterrestrial.money/mainnet/address/terra155awqc2dxvswu2sgqxlsvwm06xl3g9fw6d5vp7) |
| Mars Red Bank          | [`terra19dtgj9j5j7kyf3pmejqv8vzfpxtejaypgzkz5u`](https://finder.extraterrestrial.money/mainnet/address/terra19dtgj9j5j7kyf3pmejqv8vzfpxtejaypgzkz5u) |
| Mars Treasury          | [`terra163vehtqzfcle397aepz64hdn89m89vr297mcz2`](https://finder.extraterrestrial.money/mainnet/address/terra163vehtqzfcle397aepz64hdn89m89vr297mcz2) |
| Mars Governance        | [`terra1685de0sx5px80d47ec2xjln224phshysqxxeje`](https://finder.extraterrestrial.money/mainnet/address/terra1685de0sx5px80d47ec2xjln224phshysqxxeje) |
| Martian Field LUNA-UST | [`terra1kztywx50wv38r58unxj9p6k3pgr2ux6w5x68md`](https://finder.extraterrestrial.money/mainnet/address/terra1kztywx50wv38r58unxj9p6k3pgr2ux6w5x68md) |
| Martian Field ANC-UST  | [`terra1vapq79y9cqghqny7zt72g4qukndz282uvqwtz6`](https://finder.extraterrestrial.money/mainnet/address/terra1vapq79y9cqghqny7zt72g4qukndz282uvqwtz6) |
| Martian Field MIR-UST  | [`terra12dq4wmfcsnz6ycep6ek4umtuaj6luhfp256hyu`](https://finder.extraterrestrial.money/mainnet/address/terra12dq4wmfcsnz6ycep6ek4umtuaj6luhfp256hyu) |

### Testnet

| Contract               | Address                                                                                                                                              |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| LUNA-UST Pair          | [`terra1e49fv4xm3c2znzpxmxs0z2z6y74xlwxspxt38s`](https://finder.extraterrestrial.money/testnet/address/terra1e49fv4xm3c2znzpxmxs0z2z6y74xlwxspxt38s) |
| LUNA-UST LP Token      | [`terra1dqjpcqej9nxej80u0p56rhkrzlr6w8tp7txkmj`](https://finder.extraterrestrial.money/testnet/address/terra1dqjpcqej9nxej80u0p56rhkrzlr6w8tp7txkmj) |
| ANC Token              | [`terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc`](https://finder.extraterrestrial.money/testnet/address/terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc) |
| ANC-UST Pair           | [`terra13r3vngakfw457dwhw9ef36mc8w6agggefe70d9`](https://finder.extraterrestrial.money/testnet/address/terra13r3vngakfw457dwhw9ef36mc8w6agggefe70d9) |
| ANC-UST LP Token       | [`terra1agu2qllktlmf0jdkuhcheqtchnkppzrl4759y6`](https://finder.extraterrestrial.money/testnet/address/terra1agu2qllktlmf0jdkuhcheqtchnkppzrl4759y6) |
| MIR Token              | [`terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u`](https://finder.extraterrestrial.money/testnet/address/terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u) |
| MIR-UST Pair           | [`terra1xrt4j56mkefvhnyqqd5pgk7pfxullnkvsje7wx`](https://finder.extraterrestrial.money/testnet/address/terra1xrt4j56mkefvhnyqqd5pgk7pfxullnkvsje7wx) |
| MIR-UST LP Token       | [`terra1efmcf22aweaj3zzjhzgyghv88dda0yk4j9jp29`](https://finder.extraterrestrial.money/testnet/address/terra1efmcf22aweaj3zzjhzgyghv88dda0yk4j9jp29) |
| ASTRO Token            | [`terra1jqcw39c42mf7ngq4drgggakk3ymljgd3r5c3r5`](https://finder.extraterrestrial.money/testnet/address/terra1jqcw39c42mf7ngq4drgggakk3ymljgd3r5c3r5) |
| ASTRO-UST Pair         | [`terra1ec0fnjk2u6mms05xyyrte44jfdgdaqnx0upesr`](https://finder.extraterrestrial.money/testnet/address/terra1ec0fnjk2u6mms05xyyrte44jfdgdaqnx0upesr) |
| ASTRO-UST LP Token     | [`terra18zjm4scu5wqlskwafclxa9kpa9l3zrvju4vdry`](https://finder.extraterrestrial.money/testnet/address/terra18zjm4scu5wqlskwafclxa9kpa9l3zrvju4vdry) |
| Astro Generator        | [`terra1gjm7d9nmewn27qzrvqyhda8zsfl40aya7tvaw5`](https://finder.extraterrestrial.money/testnet/address/terra1gjm7d9nmewn27qzrvqyhda8zsfl40aya7tvaw5) |
| Mars Oracle            | [`terra108j350s2f4qprjluhup04zqggxuzhm4vzktm3f`](https://finder.extraterrestrial.money/testnet/address/terra108j350s2f4qprjluhup04zqggxuzhm4vzktm3f) |
| Mars Red Bank          | [`terra1avkm5w0gzwm92h0dlxymsdhx4l2rm7k0lxnwq7`](https://finder.extraterrestrial.money/testnet/address/terra1avkm5w0gzwm92h0dlxymsdhx4l2rm7k0lxnwq7) |
| Mars Treasury          | [`terra1ky7jek93rffwnapgx4p60f8km8f84wzz02llkc`](https://finder.extraterrestrial.money/testnet/address/terra1ky7jek93rffwnapgx4p60f8km8f84wzz02llkc) |
| Mars Governance        | [`terra1jtdz9fhrrwd8yak6e3z7utmkypvx0qf0n393c6`](https://finder.extraterrestrial.money/testnet/address/terra1jtdz9fhrrwd8yak6e3z7utmkypvx0qf0n393c6) |
| Martian Field LUNA-UST | [`terra1pkpgcqy38gyr978xfh9fx0ttq0jllzyfl05k4f`](https://finder.extraterrestrial.money/testnet/address/terra1pkpgcqy38gyr978xfh9fx0ttq0jllzyfl05k4f) |
| Martian Field ANC-UST  | [`terra1x3tu0tgsa3wuz97w2nm29fvhnhjnag00nxsgmy`](https://finder.extraterrestrial.money/testnet/address/terra1x3tu0tgsa3wuz97w2nm29fvhnhjnag00nxsgmy) |
| Martian Field MIR-UST  | [`terra1wrj7lrrxzdmcmpask6y48eudq7huvu8eylssjs`](https://finder.extraterrestrial.money/testnet/address/terra1wrj7lrrxzdmcmpask6y48eudq7huvu8eylssjs) |

## License

Contents of this repository are open source under [GNU General Public License v3](./LICENSE) or later.
