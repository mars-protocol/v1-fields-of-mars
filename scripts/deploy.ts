import * as path from "path";
import dotenv from "dotenv";
import yargs from "yargs/yargs";
import { LCDClient, MnemonicKey, Wallet } from "@terra-money/terra.js";
import { storeCode, instantiateContract } from "./helpers";

const SPECIFIC_PARAMS_MAINNET = {
  luna: {
    primary_asset_info: {
      native: "uluna",
    },
    primary_pair: {
      contract_addr: "",
      liquidity_token: "",
    },
  },
  anchor: {
    primary_asset_info: {
      cw20: "",
    },
    primary_pair: {
      contract_addr: "",
      liquidity_token: "",
    },
  },
  mirror: {
    primary_asset_info: {
      cw20: "",
    },
    primary_pair: {
      contract_addr: "",
      liquidity_token: "",
    },
  },
};

const COMMON_PARAMS_MAINNET = {
  astro_token_info: {
    cw20: "",
  },
  astro_pair: {
    contract_addr: "",
    liquidity_token: "",
  },
  astro_generator: {
    contract_addr: "",
  },
  red_bank: {
    contract_addr: "",
  },
  oracle: {
    contract_addr: "",
  },
  treasury: "",
  governance: "",
  max_ltv: "0.75",
  fee_rate: "0.05",
  bonus_rate: "0.05",
};

const SPECIFIC_PARAMS_TESTNET = {
  luna: {
    primary_asset_info: {
      native: "uluna",
    },
    primary_pair: {
      contract_addr: "terra12eq2zmdmycvx9n6skwpu9kqxts0787rekjnlwm",
      liquidity_token: "terra1sjpns87xfa48hwy6pwqdchxzsrsmmewsxjwvcj",
    },
  },
  anchor: {
    primary_asset_info: {
      cw20: "terra1yz03fpmuhf7w999fng5l5z82cufszlr92ncpzx",
    },
    primary_pair: {
      contract_addr: "terra1muj37ly5fxtjde9wz8gxe24gr0v2fdgshdx0sh",
      liquidity_token: "terra1xq64syky8wkeqsgsxm6frqpmq4p3j6jfnapkpf",
    },
  },
  mirror: {
    primary_asset_info: {
      cw20: "terra1krzfsvl9tgce2f2wsq23s0jmqqd69uwpcd3579",
    },
    primary_pair: {
      contract_addr: "terra1rnpac0n5gy38d2s440sz8dk26t9je6qa23e04g",
      liquidity_token: "terra1h5yxv4w84wjnntskz63xq3lnqqajf7xddy78qm",
    },
  },
};

const COMMON_PARAMS_TESTNET = {
  secondary_asset_info: {
    native: "uusd",
  },
  astro_token_info: {
    cw20: "terra1cc2up8erdqn2l7nz37qjgvnqy56sr38aj9vqry",
  },
  astro_pair: {
    contract_addr: "terra1dk57pl4v4ut9kwsmtrv9k4kkn9fxrh290zvg2w",
    liquidity_token: "terra1uahqpnm4p3ag8ma40xhtft96uvuxy6vn9p6x9v",
  },
  astro_generator: {
    contract_addr: "terra1cmqhxgna6uasnycgdcx974uq8u56rp2ta3r356",
  },
  red_bank: {
    contract_addr: "terra19fy8q4vx6uzv4rmhvvp329fgr5343qrunntq60",
  },
  oracle: {
    contract_addr: "terra1uxs9f90kr2lgt3tpkpyk5dllqrwra5tgwv0pc5",
  },
  treasury: "terra1u4sk8992wz4c9p5c8ckffj4h8vh97hfeyw9x5n",
  governance: "terra1w0acggjar67f7l4phnvqzeg0na0k5fcn9lv5zz",
  max_ltv: "0.75",
  fee_rate: "0.05",
  bonus_rate: "0.05",
};

function generateInitMsg(network: string, strategy: string) {
  let specificParams: typeof SPECIFIC_PARAMS_MAINNET | typeof SPECIFIC_PARAMS_TESTNET;
  let commonParams: typeof COMMON_PARAMS_MAINNET | typeof COMMON_PARAMS_TESTNET;
  if (network === "mainnet") {
    specificParams = SPECIFIC_PARAMS_MAINNET;
    commonParams = COMMON_PARAMS_MAINNET;
  } else if (network === "testnet") {
    specificParams = SPECIFIC_PARAMS_TESTNET;
    commonParams = COMMON_PARAMS_TESTNET;
  } else {
    throw new Error("invalid network: must be `mainnet` | `testnet`");
  }

  let strategySpecificParams: {
    primary_asset_info: { native: string } | { cw20: string };
    primary_pair: {
      contract_addr: string;
      liquidity_token: string;
    };
  };
  if (strategy === "luna" || strategy === "anchor" || strategy === "mirror") {
    strategySpecificParams = specificParams[strategy];
  } else {
    throw new Error("invalid strategy: must be `luna` | `anchor` | `mirror`");
  }

  return { ...strategySpecificParams, ...commonParams };
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
  console.log(`success! address: ${contractAddress}`);

  process.exit(0);
});
