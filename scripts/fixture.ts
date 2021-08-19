import * as path from "path";
import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract, sendTransaction } from "./helpers";

//----------------------------------------------------------------------------------------
// Astroport token + pair
//----------------------------------------------------------------------------------------

export async function deployAstroport(
  terra: LocalTerra,
  deployer: Wallet,
  stable = false, // whether to deploy `astroport_pair` or `astroport_pair_stable`
  nativeAsset = "uusd" // the native asset to be paired with the token
) {
  // upload binaries
  // 1. token
  process.stdout.write("Uploading Astroport token code... ");

  const tokenCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_token.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${tokenCodeId}`);

  // 2. factory
  process.stdout.write("Uploading Astroport factory code... ");

  const factoryCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/astroport_factory.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${factoryCodeId}`);

  // 3. pair
  process.stdout.write("Uploading Astroport pair code... ");

  const codePath = stable
    ? "../artifacts/astroport_pair_stable.wasm"
    : "../artifacts/astroport_pair.wasm";

  const pairCodeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${pairCodeId}`);

  // instantiate token contract
  process.stdout.write("Instantiating Astroport token contract... ");

  const tokenResult = await instantiateContract(terra, deployer, deployer, tokenCodeId, {
    name: "Test Token",
    symbol: "TEST",
    decimals: 6,
    initial_balances: [],
    mint: {
      minter: deployer.key.accAddress,
    },
  });

  const tokenAddress = tokenResult.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${tokenAddress}`);

  // instantiate factory contract
  process.stdout.write("Instantiating Astroport factory contract... ");

  const factoryResult = await instantiateContract(
    terra,
    deployer,
    deployer,
    factoryCodeId,
    {
      pair_code_ids: [pairCodeId],
      token_code_id: tokenCodeId,
      init_hook: undefined,
      fee_address: undefined,
    }
  );

  const factoryAddress = factoryResult.logs[0].events[0].attributes[3].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${factoryAddress}`);

  // create pair
  process.stdout.write("Creating Astroport pair... ");

  const pairResult = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, factoryAddress, {
      create_pair: {
        pair_code_id: pairCodeId,
        asset_infos: [
          {
            native_token: {
              denom: nativeAsset,
            },
          },
          {
            token: {
              contract_addr: tokenAddress,
            },
          },
        ],
        init_hook: undefined,
      },
    }),
  ]);

  const pairAddress = pairResult.logs[0].events[2].attributes[3].value;
  const lpTokenAddress = pairResult.logs[0].events[2].attributes[7].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${pairAddress}`);

  return {
    astroportToken: tokenAddress,
    astroportFactory: factoryAddress,
    astroportPair: pairAddress,
    astroportLpToken: lpTokenAddress,
  };
}

//----------------------------------------------------------------------------------------
// Mock Mars Liquidity Pool aka Red Bank
//----------------------------------------------------------------------------------------

export async function deployMockMars(terra: LocalTerra, deployer: Wallet) {
  process.stdout.write("Uploading Mock Mars code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mars.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Mock Mars contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {});

  const contractAddress = result.logs[0].events[0].attributes[3].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return contractAddress;
}

//----------------------------------------------------------------------------------------
// Mock Anchor Staking
//----------------------------------------------------------------------------------------

export async function deployMockAnchor(
  terra: LocalTerra,
  deployer: Wallet,
  anchorToken: string,
  astroportLpToken: string
) {
  process.stdout.write("Uploading Anchor Staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_anchor.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Anchor Staking contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    anchor_token: anchorToken,
    staking_token: astroportLpToken,
  });

  const contractAddress = result.logs[0].events[0].attributes[3].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return contractAddress;
}

//----------------------------------------------------------------------------------------
// Mock Mirror Staking
//----------------------------------------------------------------------------------------

export async function deployMockMirror(
  terra: LocalTerra,
  deployer: Wallet,
  mirrorToken: string,
  mAssetToken: string,
  astroportLpToken: string
) {
  process.stdout.write("Uploading Mirror Staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mirror.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Mirror Staking contract... ");

  const result = await instantiateContract(terra, deployer, deployer, codeId, {
    mirror_token: mirrorToken,
    asset_token: mAssetToken,
    staking_token: astroportLpToken,
  });

  const contractAddress = result.logs[0].events[0].attributes[3].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return contractAddress;
}

//----------------------------------------------------------------------------------------
// Martian Field
//----------------------------------------------------------------------------------------

export async function deployMartianField(
  terra: LocalTerra,
  deployer: Wallet,
  codePath: string,
  instantiateMsg: object
) {
  process.stdout.write("Uploading Martian Field code... ");

  const codeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write(`Instantiating Martian Field contract... `);

  const result = await instantiateContract(
    terra,
    deployer,
    deployer,
    codeId,
    instantiateMsg
  );

  const contractAddress = result.logs[0].events[0].attributes[3].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return contractAddress;
}
