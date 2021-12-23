import * as path from "path";
import dotenv from "dotenv";
import yargs from "yargs/yargs";
import { LCDClient, MnemonicKey, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract } from "./helpers";

const MAINNET_CONTRACTS = {
  anchor: {
    token: "terra14z56l0fp2lsf86zy3hty2z47ezkhnthtr9yq76",
    staking: "terra1897an2xux840p9lrh6py3ryankc6mspw49xse3",
    pair: "terra1gm5p3ner9x9xpwugn9sp6gvhd0lwrtkyrecdn3",
    shareToken: "terra1gecs98vcuktyfkrve9czrpgtg0m3aq586x6gzm",
  },
  mirror: {
    token: "terra15gwkyepfc6xgca5t5zefzwy42uts8l2m4g40k6",
    staking: "terra17f7zu97865jmknk7p2glqvxzhduk78772ezac5",
    pair: "terra1amv303y8kzxuegvurh0gug2xe9wkgj65enq2ux",
    shareToken: "terra17gjf2zehfvnyjtdgua9p9ygquk6gukxe7ucgwh",
  },
  pylon: {
    token: "",
    staking: "",
    pair: "",
    shareToken: "",
  },
  mars: {
    token: "",
    staking: "",
    pair: "",
    shareToken: "",
  },
  redBank: "",
  oracle: "",
};

const TESTNET_CONTRACTS = {
  anchor: {
    token: "terra1747mad58h0w4y589y3sk84r5efqdev9q4r02pc",
    staking: "terra19nxz35c8f7t3ghdxrxherym20tux8eccar0c3k",
    pair: "terra1wfvczps2865j0awnurk9m04u7wdmd6qv3fdnvz",
    shareToken: "terra1vg0qyq92ky9z9dp0j9fv5rmr2s80sg605dah6f",
  },
  mirror: {
    token: "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u",
    staking: "terra1a06dgl27rhujjphsn4drl242ufws267qxypptx",
    pair: "terra1cz6qp8lfwht83fh9xm9n94kj04qc35ulga5dl0",
    shareToken: "terra1zrryfhlrpg49quz37u90ck6f396l4xdjs5s08j",
  },
  pylon: {
    token: "terra1lqm5tutr5xcw9d5vc4457exa3ghd4sr9mzwdex",
    staking: "terra17av0lfhqymusm6j9jpepzerg6u54q57jp7xnrz",
    pair: "terra1n2xmlwqpp942nfqq2muxn0u0mqk3sylekdpqfv",
    shareToken: "terra1st9me79vkk4erw3apydt5z48n6ahgj4qdclp4u",
  },
  mars: {
    token: "terra1qs7h830ud0a4hj72yr8f7jmlppyx7z524f7gw6",
    staking: "terra16vc4ahvj45k8efmvz5x9k2fvrnjqcuzzs6fqf5",
    pair: "terra1lpfkyxkzhdmf80vhpyyy9esn794arqrjm73yq6",
    shareToken: "terra1rftsfyrgg5qz2268ckx5thvlhv0n26k4dfj54p",
  },
  redBank: "terra19fy8q4vx6uzv4rmhvvp329fgr5343qrunntq60",
  oracle: "terra1uxs9f90kr2lgt3tpkpyk5dllqrwra5tgwv0pc5",
};

function generateInitMsg(network: string, strategy: string) {
  let contracts: typeof MAINNET_CONTRACTS | typeof TESTNET_CONTRACTS;
  if (network === "mainnet") {
    contracts = MAINNET_CONTRACTS;
  } else if (network === "testnet") {
    contracts = TESTNET_CONTRACTS;
  } else {
    throw new Error("invalid network: must be `mainnet` | `testnet`");
  }

  let protocol: {
    token: string;
    staking: string;
    pair: string;
    shareToken: string;
  };
  let stakingType: "anchor" | "mirror" | "mars";
  if (strategy === "anchor" || strategy === "mirror" || strategy === "mars") {
    protocol = contracts[strategy];
    stakingType = strategy;
  } else if (strategy === "pylon") {
    protocol = contracts.pylon;
    // for pylon we use the anchor staking type as their staking contracts use the same messages
    stakingType = "anchor";
  } else {
    throw new Error("invalid strategy: must be `anchor` | `mirror` | `pylon` | `mars`");
  }

  return {
    primary_asset_info: {
      cw20: protocol.token,
    },
    secondary_asset_info: {
      native: "uusd",
    },
    red_bank: {
      contract_addr: contracts.redBank,
    },
    oracle: {
      contract_addr: contracts.oracle,
    },
    pair: {
      contract_addr: protocol.pair,
      liquidity_token: protocol.shareToken,
    },
    staking: {
      [stakingType]: {
        contract_addr: protocol.staking,
        asset_token: protocol.token,
        staking_token: protocol.shareToken,
      },
    },
    treasury: deployer.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.80",
    fee_rate: "0.10",
    bonus_rate: "0.05",
  };
}

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
    "fee-denom": {
      alias: "f",
      type: "string",
      default: "uusd",
      demandOption: false,
    },
  })
  .parseSync();

let terra: LCDClient;
if (argv.network === "mainnet") {
  terra = new LCDClient({
    chainID: "columbus-5",
    URL: "https://lcd.terra.dev",
  });
} else if (argv.network === "testnet") {
  terra = new LCDClient({
    chainID: "bombay-12",
    URL: "https://bombay-lcd.terra.dev",
  });
} else {
  throw new Error("invalid network: must be `mainnet` | `testnet`");
}

let deployer: Wallet;
dotenv.config();
if (!process.env.MNEMONIC) {
  throw new Error("mnemonic not provided");
} else {
  deployer = terra.wallet(
    new MnemonicKey({
      mnemonic: process.env.MNEMONIC,
    })
  );
}

console.log(`network  : ${argv.network}`);
console.log(`strategy : ${argv.strategy}`);
console.log(`codeId   : ${argv["code-id"] == 0 ? "unspecified" : argv["code-id"]}`);
console.log(`deployer : ${deployer.key.accAddress}`);

const initMsg = generateInitMsg(argv.network, argv.strategy);
console.log("instantiateMsg =", JSON.stringify(initMsg, null, 2));

process.stdout.write("ready to execute! press any key to continue, CTRL+C to abort...");
process.stdin.once("data", async function () {
  // if code id is not provided, we upload the code first
  let codeId = argv["code-id"];
  if (codeId == 0) {
    process.stdout.write("uploading contract code... ");
    codeId = await storeCode(terra, deployer, path.resolve("../artifacts/martian_field.wasm"));
    console.log(`success! codeId=${codeId}`);
  }

  // deploy the contract
  process.stdout.write("instantiating contract... ");
  const result = await instantiateContract(
    terra,
    deployer, // deployer
    deployer, // admin
    codeId,
    initMsg
  );
  const contractAddress = result.logs[0].eventsByType.instantiate_contract.contract_address[0];
  console.log(`success! contractAddress=${contractAddress}`);

  process.exit(0);
});
