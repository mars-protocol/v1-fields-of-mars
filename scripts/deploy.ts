import * as path from "path";
import chalk from "chalk";
import dotenv from "dotenv";
import yargs from "yargs/yargs";
import { LCDClient, MnemonicKey, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract } from "./helpers";

//----------------------------------------------------------------------------------------
// CONTRACT ADDRESSES
//----------------------------------------------------------------------------------------

const COLUMBUS_CONTRACTS = {
  anchor: {
    token: "terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76",
    staking: "terra1897an2xux840p9lrh6py3ryankc6mspw49xse3",
    terraswapPair: "terra1gm5p3ner9x9xpwugn9sp6gvhd0lwrtkyrecdn3",
    terraswapLpToken: "terra1gecs98vcuktyfkrve9czrpgtg0m3aq586x6gzm",
  },
  mirror: {
    token: "terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6",
    staking: "terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5",
    terraswapPair: "terra1amv303y8kzxuegvurh0gug2xe9wkgj65enq2ux",
    terraswapLpToken: "terra17gjf2zehfvnyjtdgua9p9ygquk6gukxe7ucgwh",
  },
  mars: {
    liquidityPool: "",
  },
};

const TEQUILA_CONTRACTS = {
  anchor: {
    token: "terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc",
    staking: "terra19nxz35c8f7t3ghdxrxherym20tux8eccar0c3k",
    terraswapPair: "terra1wfvczps2865j0awnurk9m04u7wdmd6qv3fdnvz",
    terraswapLpToken: "terra1vg0qyq92ky9z9dp0j9fv5rmr2s80sg605dah6f",
  },
  mirror: {
    token: "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u",
    staking: "terra1a06dgl27rhujjphsn4drl242ufws267qxypptx",
    terraswapPair: "terra1cz6qp8lfwht83fh9xm9n94kj04qc35ulga5dl0",
    terraswapLpToken: "terra1zrryfhlrpg49quz37u90ck6f396l4xdjs5s08j",
  },
  mars: {
    liquidityPool: "terra1knxh6cd43jswu3ahyx2cd9mzchynmpcqzpa65x",
  },
};

//----------------------------------------------------------------------------------------
// PARSE INPUT PARAMETERS
//----------------------------------------------------------------------------------------

// Parse .env
dotenv.config();

// Parse options
const argv = yargs(process.argv)
  .options({
    network: {
      alias: "n",
      type: "string",
      demandOption: true,
    },
    strategy: {
      alias: "s",
      type: "string",
      demandOption: true,
    },
    "code-id": {
      alias: "c",
      type: "number",
      default: 0,
      demandOption: false,
    },
  })
  .parseSync();

let deployer: Wallet;
let terra: LCDClient;
let contracts: { [key: string]: { [key: string]: string } };

if (!["columbus", "tequila"].includes(argv.network)) {
  console.log(chalk.red("Error!"), "Invalid network: must be 'columbus' or 'tequila'");
  process.exit(0);
} else {
  terra =
    argv.network == "columbus"
      ? new LCDClient({
          URL: "https://lcd.terra.dev",
          chainID: "columbus-4",
        })
      : new LCDClient({
          URL: "https://tequila-lcd.terra.dev",
          chainID: "tequila-0004",
        });

  contracts = argv.network == "columbus" ? COLUMBUS_CONTRACTS : TEQUILA_CONTRACTS;

  console.log(`Using network ${chalk.cyan(argv.network)}`);
}

if (!process.env.MNEMONIC) {
  console.log(chalk.red("Error!"), "MNEMONIC not provided");
  process.exit(0);
} else {
  deployer = terra.wallet(
    new MnemonicKey({
      mnemonic: process.env.MNEMONIC,
    })
  );
  console.log(`Using deployer ${chalk.cyan(deployer.key.accAddress)}`);
}

if (!["anchor", "mirror"].includes(argv.strategy)) {
  console.log(chalk.red("Error!"), "Invalid strategy: must be 'anchor' or 'mirror'");
  process.exit(0);
} else {
  console.log(`Using strategy ${chalk.cyan(argv.strategy)}`);
}

if (argv["code-id"] == 0) {
  console.log(
    chalk.yellow("Warning!", "Code ID not provided. Will upload contract code")
  );
} else {
  console.log(`Using code ID ${chalk.cyan(argv["code-id"])}`);
}

//----------------------------------------------------------------------------------------
// DEPLOY CONTRACT
//----------------------------------------------------------------------------------------

(async () => {
  // If CODE_ID is not provided, we upload the code first
  if (!argv["code-id"]) {
    process.stdout.write("Uploading contract code... ");

    const codeId = await storeCode(
      terra,
      deployer,
      path.resolve("../artifacts/field_of_mars_strategy.wasm")
    );

    console.log("Done!", `${chalk.blue("codeId")}=${codeId}`);
    argv["code-id"] = codeId;
  }

  // Deploy the contract
  process.stdout.write("Instantiating contract... ");

  const result = await instantiateContract(
    terra,
    deployer,
    argv["code-id"],
    {
      owner: deployer.key.accAddress,
      operators: [deployer.key.accAddress],
      treasury: deployer.key.accAddress,
      asset_token:
        argv.strategy == "anchor" ? contracts.anchor.token : contracts.mirror.token,
      reward_token:
        argv.strategy == "anchor" ? contracts.anchor.token : contracts.mirror.token,
      pool:
        argv.strategy == "anchor"
          ? contracts.anchor.terraswapPair
          : contracts.mirror.terraswapPair,
      pool_token:
        argv.strategy == "anchor"
          ? contracts.anchor.terraswapLpToken
          : contracts.mirror.terraswapLpToken,
      mars: contracts.mars.liquidityPool,
      staking_contract:
        argv.strategy == "anchor" ? contracts.anchor.staking : contracts.mirror.staking,
      staking_type: argv.strategy,
      performance_fee_rate: "0.20",
      liquidation_fee_rate: "0.05",
      liquidation_threshold: "0.67",
    },
    undefined, // coins
    true // INPORTANT: migratable needs to be set to true
  );

  console.log(
    "Done!",
    `${chalk.blue("contractAddress")}=${result.logs[0].events[1].attributes[2].value}`
  );
})();
