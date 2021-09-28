import * as path from "path";
import chalk from "chalk";
import { LCDClient, MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract, sendTransaction } from "./helpers";
import { Contract, Protocols } from "./types";

export async function deployCw20Token(terra: LCDClient, deployer: Wallet, codeId?: number) {
  if (!codeId) {
    process.stdout.write("Uploading CW20 code... ");

    codeId = await storeCode(
      terra,
      deployer,
      path.resolve(__dirname, "../artifacts/astroport_token.wasm")
    );

    console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);
  }

  process.stdout.write("Instantiating CW20 contract... ");

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
  process.stdout.write("Uploading Astroport factory code... ");

  const factoryCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_factory.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${factoryCodeId}`);

  process.stdout.write("Uploading Astroport pair code... ");

  const pairCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_pair.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${pairCodeId}`);

  process.stdout.write("Instantiating astroport factory contract... ");

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
  process.stdout.write("Instantiating Astroport pair contract... ");

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
  process.stdout.write("Uploading Red Bank code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_red_bank.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Red Bank contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {});

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployOracle(terra: LCDClient, deployer: Wallet) {
  process.stdout.write("Uploading Oracle code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_oracle.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Oracle contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {});

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}

export async function deployAnchorStaking(
  terra: LCDClient,
  deployer: Wallet,
  anchorToken: Contract,
  astroport: Protocols.Astroport
) {
  process.stdout.write("Uploading Anchor staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_anchor.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Anchor staking contract... ");

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
  mirrorToken: Contract
) {
  process.stdout.write("Uploading Mirror staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mirror.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Mirror staking contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    mirror_token: mirrorToken.address,
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
  process.stdout.write("Uploading Martian Field code... ");

  const codeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Martian Field contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, instantiateMsg);

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { codeId, address };
}
