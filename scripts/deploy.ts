import * as path from "path";
import chalk from "chalk";
import dotenv from "dotenv";
import yargs from "yargs/yargs";
import { LCDClient, MnemonicKey, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract } from "./helpers";

//----------------------------------------------------------------------------------------
// Contract Addresses
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
    redBank: "",
  },
};

const BOMBAY_CONTRACTS = {
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
    redBank: "terra1knxh6cd43jswu3ahyx2cd9mzchynmpcqzpa65x",
  },
};

//----------------------------------------------------------------------------------------
// Parse Input Parameters
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

if (!["columbus", "bombay"].includes(argv.network)) {
  console.log(chalk.red("Error!"), "Invalid network: must be 'columbus' or 'bombay'");
  process.exit(0);
} else {
  terra =
    argv.network == "columbus"
      ? new LCDClient({
          URL: "https://lcd.terra.dev",
          chainID: "columbus-5",
        })
      : new LCDClient({
          URL: "https://bombay-lcd.terra.dev",
          chainID: "bombay-0008",
        });

  contracts = argv.network == "columbus" ? COLUMBUS_CONTRACTS : BOMBAY_CONTRACTS;

  console.log(`\nNetwork  : ${chalk.cyan(argv.network)}`);
}

if (!["anchor", "mirror"].includes(argv.strategy)) {
  console.log(chalk.red("Error!"), "Invalid strategy: must be 'anchor' or 'mirror'");
  process.exit(0);
} else {
  console.log(`Strategy : ${chalk.cyan(argv.strategy)}`);
}

if (argv["code-id"] == 0) {
  console.log(`Code ID  : ${chalk.yellow("unspecified")}`);
} else {
  console.log(`Code     : ${chalk.cyan(argv["code-id"])}`);
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
  console.log(`Deployer : ${chalk.cyan(deployer.key.accAddress)}\n`);
}

//----------------------------------------------------------------------------------------
// Deploy Martian Field
//----------------------------------------------------------------------------------------

(async () => {
  // If CODE_ID is not provided, we upload the code first
  if (!argv["code-id"]) {
    process.stdout.write("Uploading contract code... ");

    const codeId = await storeCode(
      terra,
      deployer,
      path.resolve("../artifacts/martian_field.wasm")
    );

    console.log("Done!", `${chalk.blue("codeId")}=${codeId}`);
    argv["code-id"] = codeId;
  }

  // Deploy the contract
  process.stdout.write("Instantiating contract... ");

  const initMsg = {
    long_asset: {
      token: {
        contract_addr:
          argv.strategy == "anchor" ? contracts.anchor.token : contracts.mirror.token,
      },
    },
    short_asset: {
      native_token: {
        denom: "uusd",
      },
    },
    red_bank: {
      contract_addr: contracts.mars.redBank,
    },
    swap:
      argv.strategy == "anchor"
        ? {
            pair: contracts.anchor.terraswapPair,
            share_token: contracts.anchor.terraswapLpToken,
          }
        : {
            pair: contracts.mirror.terraswapPair,
            share_token: contracts.mirror.terraswapLpToken,
          },
    staking:
      argv.strategy == "anchor"
        ? {
            anchor: {
              contract_addr: contracts.anchor.staking,
              asset_token: contracts.anchor.token,
              staking_token: contracts.anchor.terraswapLpToken,
            },
          }
        : {
            mirror: {
              contract_addr: contracts.mirror.staking,
              asset_token: contracts.mirror.token,
              staking_token: contracts.mirror.terraswapLpToken,
            },
          },
    keepers: [deployer.key.accAddress],
    treasury: deployer.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.67",
    fee_rate: "0.10",
  };

  const result = await instantiateContract(
    terra,
    deployer,
    argv["code-id"],
    initMsg,
    undefined, // coins
    true // INPORTANT: migratable needs to be set to true
  );

  console.log(
    "Done!",
    `${chalk.blue("contractAddress")}=${result.logs[0].events[0].attributes[2].value}`
  );

  console.log("\nInitMsg =", initMsg, "\n");
})();
