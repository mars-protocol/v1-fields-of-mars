import * as path from "path";
import chalk from "chalk";
import { LCDClient, MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract, sendTransaction } from "./helpers";
import { Contract, Astroport } from "./types";

export async function deployCw20Token(terra: LCDClient, deployer: Wallet) {
  process.stdout.write("CW20: uploading code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_token.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("CW20: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    name: "Test Token",
    symbol: "TEST",
    decimals: 6,
    initial_balances: [],
    mint: {
      minter: deployer.key.accAddress,
    },
  });

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployAstroport(terra: LCDClient, deployer: Wallet, cw20Token: Contract) {
  process.stdout.write("Astroport: factory: uploading code... ");

  const factoryCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_factory.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${factoryCodeId}`);

  process.stdout.write("Astroport: pair: uploading code... ");

  const pairCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_pair.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${pairCodeId}`);

  process.stdout.write("Astroport: factory: instantiating contract... ");

  const factoryResult = await instantiateContract(terra, deployer, deployer, factoryCodeId, {
    pair_configs: [
      {
        code_id: pairCodeId,
        pair_type: {
          xyk: {},
        },
        total_fee_bps: 30, // 30 bps = 0.3%
        maker_fee_bps: 0,
      },
    ],
    token_code_id: cw20Token.codeId,
  });

  const factoryAddress = factoryResult.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${factoryAddress}`);

  // create pair
  process.stdout.write("Astroport: pair: instantiating contract... ");

  const pairResult = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, factoryAddress, {
      create_pair: {
        pair_type: {
          xyk: {},
        },
        asset_infos: [
          {
            native_token: {
              denom: "uusd",
            },
          },
          {
            token: {
              contract_addr: cw20Token.address,
            },
          },
        ],
      },
    }),
  ]);

  const pairAddress = pairResult.logs[0].events[2].attributes[3].value;
  const shareTokenAddress = pairResult.logs[0].events[2].attributes[7].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("pair.address")}=${pairAddress}`,
    `${chalk.blue("shareToken.address")}=${shareTokenAddress}`
  );

  return {
    factory: {
      codeId: factoryCodeId,
      address: factoryAddress,
    },
    pair: {
      codeId: pairCodeId,
      address: pairAddress,
    },
    shareToken: {
      codeId: cw20Token.codeId,
      address: shareTokenAddress,
    },
  };
}

export async function deployRedBank(terra: LCDClient, deployer: Wallet) {
  process.stdout.write("Mars: Red Bank: uploading code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_red_bank.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Mars: Red Bank: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {});

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployOracle(terra: LCDClient, deployer: Wallet, astroport: Astroport) {
  process.stdout.write("Mars: Oracle: uploading code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_oracle.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Mars: Oracle: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {});

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployAnchorStaking(
  terra: LCDClient,
  deployer: Wallet,
  anchorToken: Contract,
  astroport: Astroport
) {
  process.stdout.write("Anchor: staking: uploading code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_anchor.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Anchor: staking: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    anchor_token: anchorToken.address,
    staking_token: astroport.shareToken.address,
  });

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployMirrorStaking(
  terra: LCDClient,
  deployer: Wallet,
  mirrorToken: Contract,
  assetToken: Contract,
  astroport: Astroport
) {
  process.stdout.write("Mirror: staking: uploading code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mirror.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Mirror: staking: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    mirror_token: mirrorToken.address,
    asset_token: assetToken.address,
    staking_token: astroport.shareToken.address,
  });

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployMartianField(
  terra: LCDClient,
  deployer: Wallet,
  codePath: string,
  instantiateMsg: object
) {
  process.stdout.write("Martian Field: uploading code... ");

  const codeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Martian Field: instantiating contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, instantiateMsg);

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}
