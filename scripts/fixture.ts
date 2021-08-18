import * as path from "path";
import chalk from "chalk";
import { LocalTerra, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract } from "./helpers";

//----------------------------------------------------------------------------------------
// CW20 token
//----------------------------------------------------------------------------------------

export async function deployAstroportToken(
  terra: LocalTerra,
  deployer: Wallet,
  name: string,
  symbol: string,
  decimals?: number,
  cw20CodeId?: number
) {
  if (!cw20CodeId) {
    process.stdout.write("CW20 code ID not given! Uploading CW20 code... ");

    cw20CodeId = await storeCode(
      terra,
      deployer,
      path.resolve(__dirname, "../artifacts/astroport_token.wasm")
    );

    console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${cw20CodeId}`);
  }

  process.stdout.write(`Instantiating ${symbol} token contract... `);

  const result = await instantiateContract(terra, deployer, deployer, cw20CodeId, {
    name: name,
    symbol: symbol,
    decimals: decimals ? decimals : 6,
    initial_balances: [],
    mint: {
      minter: deployer.key.accAddress,
    },
  });

  const contractAddress = result.logs[0].events[0].attributes[3].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return {
    cw20CodeId,
    cw20Token: contractAddress,
  };
}

//----------------------------------------------------------------------------------------
// Astroport Pair
//----------------------------------------------------------------------------------------

export async function deployAstroportPair(
  terra: LocalTerra,
  deployer: Wallet,
  instantiateMsg: object,
  stable = false // whether to deploy `astroport_pair` or `astroport_pair_stable`
) {
  process.stdout.write("Uploading Astroport pair code... ");

  const codePath = stable
    ? "../artifacts/astroport_pair_stable.wasm"
    : "../artifacts/astroport_pair.wasm";

  const codeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Astroport pair contract... ");

  const result = await instantiateContract(
    terra,
    deployer,
    deployer,
    codeId,
    instantiateMsg
  );

  const event = result.logs[0].events.find((event) => {
    return event.type == "instantiate_contract";
  });

  const astroportPair = event?.attributes[3].value;
  const astroportLpToken = event?.attributes[7].value;

  if (!astroportPair || !astroportLpToken) {
    throw "failed to parse instantiation event log";
  }

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("astroportPair")}=${astroportPair}`,
    `${chalk.blue("astroportLpToken")}=${astroportLpToken}`
  );

  return { astroportPair, astroportLpToken };
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
