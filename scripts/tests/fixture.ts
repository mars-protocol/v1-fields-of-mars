import * as path from "path";
import chalk from "chalk";
import { MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract, sendTransaction } from "../helpers/tx";

//--------------------------------------------------------------------------------------------------
// CW20
//--------------------------------------------------------------------------------------------------

export async function deployCw20Token(
  deployer: Wallet,
  codeId?: number,
  name?: string,
  symbol?: string,
  decimals?: number
) {
  name = name ? name : "Test Token";
  symbol = symbol ? symbol : "TEST";
  decimals = decimals ? decimals : 6;

  if (!codeId) {
    process.stdout.write("Uploading CW20 code... ");

    codeId = await storeCode(
      deployer,
      path.resolve(__dirname, "../../artifacts/astroport_token.wasm")
    );

    console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);
  }

  process.stdout.write(`Instantiating ${symbol} token contract... `);

  const result = await instantiateContract(deployer, codeId, {
    name,
    symbol,
    decimals,
    initial_balances: [],
    mint: {
      minter: deployer.key.accAddress,
    },
  });

  const address = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${address}`);

  return { cw20CodeId: codeId, address };
}

//--------------------------------------------------------------------------------------------------
// Astroport factory
//--------------------------------------------------------------------------------------------------

export async function deployAstroportFactory(deployer: Wallet, cw20CodeId: number) {
  process.stdout.write("Uploading Astroport factory code... ");

  const factoryCodeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/astroport_factory.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${factoryCodeId}`);

  process.stdout.write("Uploading Astroport pair code... ");

  const pairCodeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/astroport_pair.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${pairCodeId}`);

  process.stdout.write("Instantiating Astroport factory contract... ");

  const result = await instantiateContract(deployer, factoryCodeId, {
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
    token_code_id: cw20CodeId,
    owner: deployer.key.accAddress,
  });

  const astroportFactory = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${astroportFactory}`);

  return { factoryCodeId, pairCodeId, astroportFactory };
}

//--------------------------------------------------------------------------------------------------
// Astroport pair
//--------------------------------------------------------------------------------------------------

export async function deployAstroportPair(
  deployer: Wallet,
  astroportFactory: string,
  legacyAssetInfos: object
) {
  process.stdout.write("Creating Astroport pair... ");

  const result = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroportFactory, {
      create_pair: {
        pair_type: {
          xyk: {},
        },
        asset_infos: legacyAssetInfos,
      },
    }),
  ]);

  const astroportPair = result.logs[0].events[2].attributes[3].value;
  const astroportLpToken = result.logs[0].events[2].attributes[7].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${astroportPair}`);

  return { astroportPair, astroportLpToken };
}

//--------------------------------------------------------------------------------------------------
// Mock Astro generator
//--------------------------------------------------------------------------------------------------

export async function deployAstroGenerator(
  deployer: Wallet,
  liquidityToken: string,
  astroToken: string,
  proxyRewardToken?: string
) {
  process.stdout.write("Uploading mock Astroport generator code... ");

  const codeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/mock_astro_generator.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stderr.write("Instantiating mock Astroport generator contract... ");

  const result = await instantiateContract(deployer, codeId, {
    liquidity_token: liquidityToken,
    astro_token: astroToken,
    proxy_reward_token: proxyRewardToken,
  });

  const astroGenerator = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${astroGenerator}`);

  return { generatorCodeId: codeId, astroGenerator };
}

//--------------------------------------------------------------------------------------------------
// Mock oracle
//--------------------------------------------------------------------------------------------------

export async function deployOracle(deployer: Wallet) {
  process.stdout.write("Uploading mock Oracle code... ");

  const codeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/mock_oracle.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating mock Oracle contract... ");

  const result = await instantiateContract(deployer, codeId, {});

  const oracle = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${oracle}`);

  return { oracleCodeId: codeId, oracle };
}

//--------------------------------------------------------------------------------------------------
// Mock Red Bank
//--------------------------------------------------------------------------------------------------

export async function deployRedBank(deployer: Wallet) {
  process.stdout.write("Uploading mock Red Bank code... ");

  const codeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/mock_red_bank.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating mock Red Bank contract... ");

  const result = await instantiateContract(deployer, codeId, {});

  const bank = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${bank}`);

  return { bankCodeId: codeId, bank };
}

//--------------------------------------------------------------------------------------------------
// Martian Field
//--------------------------------------------------------------------------------------------------

export async function deployMartianField(deployer: Wallet, instantiateMsg: object) {
  process.stdout.write("Uploading Martian Field code... ");

  const codeId = await storeCode(
    deployer,
    path.resolve(__dirname, "../../artifacts/martian_field.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Martian Field contract... ");

  const result = await instantiateContract(deployer, codeId, instantiateMsg);

  const field = result.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("address")}=${field}`);

  return { martianFieldCodeId: codeId, field };
}
