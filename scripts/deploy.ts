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
    astroportPair: "terra1gm5p3ner9x9xpwugn9sp6gvhd0lwrtkyrecdn3",
    astroportLpToken: "terra1gecs98vcuktyfkrve9czrpgtg0m3aq586x6gzm",
  },
  mirror: {
    token: "terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6",
    staking: "terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5",
    astroportPair: "terra1amv303y8kzxuegvurh0gug2xe9wkgj65enq2ux",
    astroportLpToken: "terra17gjf2zehfvnyjtdgua9p9ygquk6gukxe7ucgwh",
  },
  mars: {
    token: "",
    staking: "",
    astroportPair: "",
    astroportLpToken: "",
  },
  redBank: "",
};

const BOMBAY_CONTRACTS = {
  anchor: {
    token: "terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc",
    staking: "terra19nxz35c8f7t3ghdxrxherym20tux8eccar0c3k",
    astroportPair: "terra1wfvczps2865j0awnurk9m04u7wdmd6qv3fdnvz",
    astroportLpToken: "terra1vg0qyq92ky9z9dp0j9fv5rmr2s80sg605dah6f",
  },
  mirror: {
    token: "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u",
    staking: "terra1a06dgl27rhujjphsn4drl242ufws267qxypptx",
    astroportPair: "terra1cz6qp8lfwht83fh9xm9n94kj04qc35ulga5dl0",
    astroportLpToken: "terra1zrryfhlrpg49quz37u90ck6f396l4xdjs5s08j",
  },
  mars: {
    token: "",
    staking: "",
    astroportPair: "",
    astroportLpToken: "",
  },
  redBank: "",
};

const INSTANTIATE_MSG = (
  contracts: {
    token: string;
    staking: string;
    astroportPair: string;
    astroportLpToken: string;
  },
  redBank: string
) => {
  return {
    long_asset: {
      token: {
        contract_addr: contracts.token,
      },
    },
    short_asset: {
      native_token: {
        denom: "uusd",
      },
    },
    red_bank: {
      contract_addr: redBank,
    },
    swap: {
      pair: contracts.astroportPair,
      share_token: contracts.astroportLpToken,
    },
    staking: {
      anchor: {
        contract_addr: contracts.staking,
        asset_token: contracts.token,
        staking_token: contracts.astroportLpToken,
      },
    },
    keepers: [deployer.key.accAddress],
    treasury: deployer.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.67",
    fee_rate: "0.10",
  };
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
let contracts: typeof COLUMBUS_CONTRACTS | typeof BOMBAY_CONTRACTS;

if (!["columbus", "bombay"].includes(argv.network)) {
  console.log(chalk.red("Error!"), "invalid network: must be 'columbus' or 'bombay'");
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
          chainID: "bombay-11",
        });

  contracts = argv.network == "columbus" ? COLUMBUS_CONTRACTS : BOMBAY_CONTRACTS;

  console.log(`\nnetwork  : ${chalk.cyan(argv.network)}`);
}

if (!["anchor", "mirror", "mars"].includes(argv.strategy)) {
  console.log(
    chalk.red("Error!"),
    "Invalid strategy: must be 'anchor' | 'mirror' | 'mars'"
  );
  process.exit(0);
} else {
  console.log(`strategy : ${chalk.cyan(argv.strategy)}`);
}

if (argv["code-id"] == 0) {
  console.log(`code     : ${chalk.yellow("unspecified")}`);
} else {
  console.log(`code     : ${chalk.cyan(argv["code-id"])}`);
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
  console.log(`deployer : ${chalk.cyan(deployer.key.accAddress)}\n`);
}

//----------------------------------------------------------------------------------------
// Deploy Martian Field
//----------------------------------------------------------------------------------------

const instantiateMsg = INSTANTIATE_MSG(
  argv.strategy === "anchor"
    ? contracts.anchor
    : argv.strategy === "mirror"
    ? contracts.mirror
    : contracts.mars,
  contracts.redBank
);

console.log("instantiateMsg =", instantiateMsg, "\n");

process.stdout.write("Ready to deploy; press any key to continue, CTRL+C to abort...");

process.stdin.once("data", async function () {
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
  process.stdout.write("Instantiating Martian Field... ");

  const result = await instantiateContract(
    terra,
    deployer, // deployer
    deployer, // admin
    argv["code-id"],
    instantiateMsg
  );

  console.log(
    "Done!",
    `${chalk.blue("contractAddress")}=${result.logs[0].events[0].attributes[3].value}\n`
  );

  process.exit(0);
});
