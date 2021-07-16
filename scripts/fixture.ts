import * as path from "path";
import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract, sendTransaction } from "./helpers";

//----------------------------------------------------------------------------------------
// CW20 token
//----------------------------------------------------------------------------------------

export async function deployTerraswapToken(
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
      path.resolve(__dirname, "../artifacts/terraswap_token.wasm")
    );

    console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${cw20CodeId}`);
  }

  process.stdout.write(`Instantiating ${symbol} token contract... `);

  const result = await instantiateContract(terra, deployer, cw20CodeId, {
    name: name,
    symbol: symbol,
    decimals: decimals ? decimals : 6,
    initial_balances: [],
    mint: {
      minter: deployer.key.accAddress,
    },
  });

  const contractAddress = result.logs[0].events[0].attributes[2].value;

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
// TerraSwap Pair
//----------------------------------------------------------------------------------------

export async function deployTerraswapPair(
  terra: LocalTerra,
  deployer: Wallet,
  cw20CodeId: number,
  cw20Token: string
) {
  process.stdout.write("Uploading TerraSwap pair code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/terraswap_pair.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating TerraSwap pair contract... ");

  const result = await instantiateContract(terra, deployer, codeId, {
    asset_infos: [
      { native_token: { denom: "uusd" } },
      { token: { contract_addr: cw20Token } },
    ],
    token_code_id: cw20CodeId,
  });

  const terraswapLpToken = result.logs[0].events[2].attributes[2].value;
  const terraswapPair = result.logs[0].events[2].attributes[5].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("terraswapPair")}=${terraswapPair}`,
    `${chalk.blue("terraswapLpToken")}=${terraswapLpToken}`
  );

  return { terraswapPair, terraswapLpToken };
}

//----------------------------------------------------------------------------------------
// Mock Mars Liquidity Pool
//----------------------------------------------------------------------------------------

export async function deployMockMars(
  terra: LocalTerra,
  deployer: Wallet,
  mockInterestRate: string = "1"
) {
  process.stdout.write("Uploading Mock Mars code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mars.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Mock Mars contract... ");

  const result = await instantiateContract(
    terra,
    deployer,
    codeId,
    {
      mock_interest_rate: mockInterestRate,
    },
    undefined, // no coin to send upon instantiation
    true // set the contract to be migratable
  );

  const redBank = result.logs[0].events[0].attributes[2].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${redBank}`);

  return redBank;
}

//----------------------------------------------------------------------------------------
// Mock Anchor Staking
//----------------------------------------------------------------------------------------

export async function deployMockAnchor(
  terra: LocalTerra,
  deployer: Wallet,
  anchorToken: string,
  terraswapLpToken: string
) {
  process.stdout.write("Uploading Anchor Staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_anchor.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Anchor Staking contract... ");

  const result = await instantiateContract(terra, deployer, codeId, {
    anchor_token: anchorToken,
    staking_token: terraswapLpToken,
  });

  const anchorStaking = result.logs[0].events[0].attributes[2].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${anchorStaking}`);

  return anchorStaking;
}

//----------------------------------------------------------------------------------------
// Mock Mirror Staking
//----------------------------------------------------------------------------------------

export async function deployMockMirror(
  terra: LocalTerra,
  deployer: Wallet,
  mirrorToken: string,
  mAssetToken: string,
  terraswapLpToken: string
) {
  process.stdout.write("Uploading Mirror Staking code... ");

  const codeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/mock_mirror.wasm")
  );

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write("Instantiating Mirror Staking contract... ");

  const result = await instantiateContract(terra, deployer, codeId, {
    mirror_token: mirrorToken,
    asset_token: mAssetToken,
    staking_token: terraswapLpToken,
  });

  const mirrorStaking = result.logs[0].events[0].attributes[2].value;

  console.log(chalk.green("Done!"), `${chalk.blue("contractAddress")}=${mirrorStaking}`);

  return mirrorStaking;
}

//----------------------------------------------------------------------------------------
// Martian Field
//----------------------------------------------------------------------------------------

export async function deployMartianField(
  terra: LocalTerra,
  deployer: Wallet,
  codePath: string,
  initMsg: object
) {
  process.stdout.write("Uploading Martian Field code... ");

  const codeId = await storeCode(terra, deployer, path.resolve(__dirname, codePath));

  console.log(chalk.green("Done!"), `${chalk.blue("codeId")}=${codeId}`);

  process.stdout.write(`Instantiating Martian Field contract... `);

  const result = await instantiateContract(
    terra,
    deployer,
    codeId,
    initMsg,
    undefined, // initCoins
    true // IMPORTANT: migratable set to true
  );

  const contractAddress = result.logs[0].events[2].attributes[2].value;

  console.log(
    chalk.green("Done!"),
    `${chalk.blue("contractAddress")}=${contractAddress}`
  );

  return contractAddress;
}
