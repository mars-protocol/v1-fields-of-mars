import chalk from "chalk";
import { expect } from "chai";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction } from "../helpers/tx";
import { encodeBase64 } from "../helpers/encoding";
import {
  deployCw20Token,
  deployAstroportFactory,
  deployAstroportPair,
  deployAstroGenerator,
  deployRedBank,
  deployOracle,
  deployMartianField,
} from "./fixture";
import { Verifier } from "./verifier";
import { Config, PositionsResponse } from "./types";

// LocalTerra instance
const terra = new LocalTerra();

// User addresses
const deployer = terra.wallets.test1;
const treasury = terra.wallets.test2;
const user1 = terra.wallets.test3;
const user2 = terra.wallets.test4;
const liquidator = terra.wallets.test5;

let anchorToken: string;
let astroToken: string;
let astroportFactory: string;
let ancUstPair: string;
let ancUstLpToken: string;
let astroUstPair: string;
let astroUstLpToken: string;
let astroGenerator: string;
let oracle: string;
let bank: string;
let field: string;

let config: Config;

let verifier: Verifier;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  const { cw20CodeId, address } = await deployCw20Token(deployer, undefined, "Anchor Token", "ANC");
  anchorToken = address;

  const result = await deployCw20Token(deployer, cw20CodeId, "Astroport Token", "ASTRO");
  astroToken = result.address;

  ({ astroportFactory } = await deployAstroportFactory(deployer, cw20CodeId));

  let { astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      native_token: {
        denom: "uusd",
      },
    },
    {
      token: {
        contract_addr: anchorToken,
      },
    },
  ]);
  ancUstPair = astroportPair;
  ancUstLpToken = astroportLpToken;

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      native_token: {
        denom: "uusd",
      },
    },
    {
      token: {
        contract_addr: astroToken,
      },
    },
  ]));
  astroUstPair = astroportPair;
  astroUstLpToken = astroportLpToken;

  ({ astroGenerator } = await deployAstroGenerator(
    deployer,
    ancUstLpToken,
    astroToken,
    anchorToken
  ));

  ({ oracle } = await deployOracle(deployer));

  ({ bank } = await deployRedBank(deployer));

  config = {
    primary_asset_info: {
      cw20: anchorToken,
    },
    secondary_asset_info: {
      native: "uusd",
    },
    astro_token_info: {
      cw20: astroToken,
    },
    primary_pair: {
      contract_addr: ancUstPair,
      liquidity_token: ancUstLpToken,
    },
    astro_pair: {
      contract_addr: astroUstPair,
      liquidity_token: astroUstLpToken,
    },
    astro_generator: {
      contract_addr: astroGenerator,
    },
    red_bank: {
      contract_addr: bank,
    },
    oracle: {
      contract_addr: oracle,
    },
    treasury: treasury.key.accAddress,
    governance: deployer.key.accAddress,
    operators: [deployer.key.accAddress],
    max_ltv: "0.75", // 75%, i.e. for every 100 UST asset there must be no more than 75 UST debt
    fee_rate: "0.2", // 20%
    bonus_rate: "0.05", // 5%
  };

  ({ field } = await deployMartianField(deployer, config));

  process.stdout.write("Configuring ANC and UST price oracle...");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          native: {
            denom: "uusd",
          },
        },
        price_source: {
          fixed: {
            price: "1",
          },
        },
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          cw20: {
            contract_addr: anchorToken,
          },
        },
        price_source: {
          astroport_spot: {
            pair_address: ancUstPair,
            asset_address: anchorToken,
          },
        },
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund deployer and users with ANC... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "69000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "34500000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund deployer with ASTRO... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Astro generator contract with ANC and ASTRO... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: astroGenerator,
        amount: "100000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: astroGenerator,
        amount: "100000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Mars contract with UST...");
  await sendTransaction(deployer, [
    new MsgSend(deployer.key.accAddress, bank, { uusd: 100000000000 }),
  ]);
  console.log(chalk.green("Done!"));

  // deployer provides 69 ANC + 420 UST
  // should receive sqrt(69000000 * 420000000) = 170235131 uLP
  process.stdout.write("Provide initial liquidity to ANC-UST Pair... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: ancUstPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      ancUstPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "69000000",
              info: {
                token: {
                  contract_addr: anchorToken,
                },
              },
            },
            {
              amount: "420000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
          ],
        },
      },
      {
        uusd: "420000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // deployer provides 100 ASTRO + 150 UST
  // should receive sqrt(100000000 * 150000000) = 122474487 uLP
  process.stdout.write("Provide initial liquidity to ASTRO-UST Pair... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      increase_allowance: {
        amount: "100000000",
        spender: astroUstPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      astroUstPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "100000000",
              info: {
                token: {
                  contract_addr: astroToken,
                },
              },
            },
            {
              amount: "150000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
          ],
        },
      },
      {
        uusd: "150000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // initialize the verifier object
  verifier = new Verifier(terra, field, config);
}

//--------------------------------------------------------------------------------------------------
// Test 1. Config
//--------------------------------------------------------------------------------------------------

async function testConfig() {
  console.log("\n1. Config...");

  await verifier.verify({
    bond: "0",
    debt: "0",
    primaryPair: {
      assets: [
        { amount: "420000000" }, // uusd
        { amount: "69000000" }, // uANC
      ],
      total_share: "170235131",
    },
    astroPair: {
      assets: [
        { amount: "150000000" }, // uusd
        { amount: "100000000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "0",
      total_debt_units: "0",
      pending_rewards: [],
    },
    users: [],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 2. Open Position, Pt. 1
//
// Prior to execution:
// ---
// bond                   0
// debt                   0
// primary pool uANC      69000000
// primary pool uusd      420000000
// primary pool uLP       170235131
// astro pool uASTRO      100000000
// astro pool uusd        150000000
// astro pool uLP         122474487
//
// Step 1. deposit
// contract receives 69000000 uANC + 0 uusd
// ---
// user1 unlocked uANC    0 + 69000000 = 69000000
//
// Step 2. borrow
// borrow 420000000 uusd
// ---
// total debt units       0 + 420000000000000 = 420000000000000
// user1 debt units       0 + 420000000000000 = 420000000000000
// user1 unlocked uusd    0 + 420000000 = 420000000
// debt                   0 + 420000000 = 420000000
//
// Step 3. provide liquidity
// sends 69000000 uANC + 420000000 uusd to primary pool
// mint amount = min(170235131 * 69000000 / 69000000, 170235131 * 420000000 / 420000000) = 170235131 uLP
// ---
// user1 unlocked uANC    69000000 - 69000000 = 0
// user1 unlocked uusd    420000000 - 420000000 = 0
// user1 unlocked uLP     0 + 170235131 = 170235131
// primary pool uANC      69000000 + 69000000 = 138000000
// primary pool uusd      420000000 + 420000000 = 840000000
// primary pool uLP       170235131 + 170235131 = 340470262
//
// Step 4. bond
// send 170235131 uLP to Astro generator
// contract should receive 1000000 uASTRO + 500000 uANC
// ---
// total bond units       0 + 170235131000000 = 170235131000000
// user1 bond units       0 + 170235131000000 = 170235131000000
// user1 unlocked uLP     170235131 - 170235131 = 0
// bond                   0 + 170235131 = 170235131
// pending reward uASTRO  0 + 1000000 = 1000000
// pending reward uANC    0 + 500000 = 500000
//
// Result
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   170235131
// debt                   420000000
// primary pool uANC      138000000
// primary pool uusd      840000000
// primary pool uLP       340470262
// astro pool uASTRO      100000000
// astro pool uusd        150000000
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 138000000, 840000000) / 1000000 = 6.043165
// primary value = 138000000 * 6.043165 = 833956770
// secondary value = 840000000 * 1 = 840000000
// pool value = 2 * sqrt(833956770 * 840000000) = 1673945860
// total bond value = 1673945860 * 170235131 / 340470262 = 836972930
// total debt value = 420000000
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 420000000 / 836972930 = 0.501808344028521926
//--------------------------------------------------------------------------------------------------

async function testOpenPosition1() {
  process.stdout.write("\n2. Opening position for user 1... ");
  const { txhash } = await sendTransaction(user1, [
    new MsgExecuteContract(user1.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: field,
      },
    }),
    new MsgExecuteContract(user1.key.accAddress, field, {
      update_position: [
        {
          deposit: {
            info: {
              cw20: anchorToken,
            },
            amount: "69000000",
          },
        },
        {
          borrow: {
            amount: "420000000",
          },
        },
        {
          bond: {
            slippage_tolerance: "0.005", // 0.5%
          },
        },
      ],
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "170235131",
    debt: "420000000",
    primaryPair: {
      assets: [
        { amount: "840000000" }, // uusd
        { amount: "138000000" }, // uANC
      ],
      total_share: "340470262",
    },
    astroPair: {
      assets: [
        { amount: "150000000" }, // uusd
        { amount: "100000000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "170235131000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1000000" }, // uASTRO
        { amount: "500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "170235131000000",
          debt_units: "420000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "170235131",
          bond_value: "836972930",
          debt_amount: "420000000",
          debt_value: "420000000",
          ltv: "0.501808344028521926",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 3. Harvest
//
// Prior to execution:
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   170235131
// debt                   420000000
// primary pool uANC      138000000
// primary pool uusd      840000000
// primary pool uLP       340470262
// astro pool uASTRO      100000000
// astro pool uusd        150000000
// astro pool uLP         122474487
//
// Step 1. claim reward
// should receive 1000000 uASTRO + 500000 uANC
// ---
// pending reward uASTRO  1000000 + 1000000 = 2000000
// pending reward uANC    500000 + 500000 = 1000000
//
// Step 2. charge fees
// ASTRO fee = 2000000 * 0.2 = 400000 uASTRO
// ANC fee = 1000000 * 0.2 = 200000 uANC
// ---
// pending reward uASTRO  2000000 - 400000 = 1600000
// pending reward uANC    1000000 - 200000 = 800000
//
// Step 3. swap ASTRO >> UST
// return amount = computeXykSwapOutput(1600000, 100000000, 150000000) = 2355118 uusd
// ---
// pending reward uASTRO  1600000 - 1600000 = 0
// pending reward uusd    0 + 2355118 = 2355118
// astro pool uASTRO      100000000 + 1600000 = 101600000
// astro pool uusd        150000000 - 2355118 = 147644882
//
// Step 4. balance
// ANC price = computeXykSwapOutput(1000000, 138000000, 840000000) / 1e6 = 6.043165
// ANC value = 800000 * 6.043165 = 4834532
// UST value = 2355118 * 1 = 2355118
// value diff = 4834532 - 2355118 = 2479414
// value to swap = 2479414 / 2 = 1239707
// amount to swap = 800000 * 1239707 / 4834532 = 205142
// UST return amount = computeXykSwapOutput(205142, 138000000, 840000000) = 1243096
// ---
// pending reward uANC    800000 - 205142 = 594858
// pending reward uusd    2355118 + 1243096 = 3598214
// primary pool uANC      138000000 + 205142 = 138205142
// primary pool uusd      840000000 - 1243096 = 838756904
//
// Step 2. provide liquidity
// sends 594858 uANC + 3598214 uusd to pool
// shares minted = min(340470262 * 594858 / 138205142, 340470262 * 3598214 / 838756904) = 1460595 uLP
// ---
// pending reward uANC    594858 - 594858 = 0
// pending reward uusd    3598214 - 3598214 = 0
// pending reward uLP     0 + 1460593 = 1460593
// primary pool uANC      138205142 + 594858 = 138800000
// primary pool uusd      838756904 + 3598214 = 842355118
// primary pool uLP       340470262 + 1460595 = 341930857
//
// Step 4. bond
// send 1460595 uLP to staking contract
// bond units should not change in a harvest transaction
// when we bond, we receive another 1000000 uASTRO + 500000 uANC
// NOTE: this is not how the actual Astro generator will behave (since we already claimed rewards in
// the same tx) but still, our contract should account for this propoerly
// ---
// pending reward uLP     1460595 - 1460595 = 0
// bond                   170235131 + 1460595 = 171695726
// pending reward uASTRO  0 + 1000000 = 1000000
// pending reward uANC    0 + 500000 = 500000
//
// Result
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   171695726
// debt                   420000000
// primary pool uANC      138800000
// primary pool uusd      842355118
// primary pool uLP       341930857
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 138800000, 842355118) / 1000000 = 6.025430
// primary value = 138800000 * 6.025430 = 836329684
// secondary value = 842355118 * 1 = 842355118
// pool value = 2 * sqrt(836329684 * 842355118) = 1678673988
// total bond value = 1678673988 * 171695726 / 341930857 = 842922313
// total debt value = 420000000
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 420000000 / 842922313 = 0.498266558522101905
//--------------------------------------------------------------------------------------------------

async function testHarvest() {
  process.stdout.write("\n3. Harvesting... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, field, {
      harvest: {
        max_spread: "0.02", // if not specified, Astroport applied a default max spread of 0.5%
        slippage_tolerance: undefined,
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "171695726",
    debt: "420000000",
    primaryPair: {
      assets: [
        { amount: "842355118" }, // uusd
        { amount: "138800000" }, // uANC
      ],
      total_share: "341930857",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "170235131000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1000000" }, // uASTRO
        { amount: "500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "170235131000000",
          debt_units: "420000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "171695726",
          bond_value: "842922313",
          debt_amount: "420000000",
          debt_value: "420000000",
          ltv: "0.498266558522101905",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 4. Accrue Interest
//
// Prior to execution:
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   171695726
// debt                   420000000
// primary pool uANC      138800000
// primary pool uusd      842355118
// primary pool uLP       341930857
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// We forcibly set the strategy's debt to 441000000 to simulate accrual of a 5% interest
//
// Result
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   171695726
// debt                   441000000
// primary pool uANC      138800000
// primary pool uusd      842355118
// primary pool uLP       341930857
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = 6.025430 (unchanged)
// total bond value = 842922313 (unchanged)
// total debt value = 441000000 uusd
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 441000000 / 842922313 = 0.523179886448207001
//--------------------------------------------------------------------------------------------------

async function testAccrueInterest() {
  process.stdout.write("\n4. Accruing interest... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bank, {
      set_user_debt: {
        user_address: field,
        denom: "uusd",
        amount: "441000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "171695726",
    debt: "441000000",
    primaryPair: {
      assets: [
        { amount: "842355118" }, // uusd
        { amount: "138800000" }, // uANC
      ],
      total_share: "341930857",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "170235131000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1000000" }, // uASTRO
        { amount: "500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "170235131000000",
          debt_units: "420000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "171695726",
          bond_value: "842922313",
          debt_amount: "441000000",
          debt_value: "441000000",
          ltv: "0.523179886448207001",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 5. Open Position, Pt. 2
//
// Prior to execution:
// ---
// total bond units       170235131000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   171695726
// debt                   441000000
// primary pool uANC      138800000
// primary pool uusd      842355118
// primary pool uLP       341930857
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// Step 1. deposit
// contract receives 34500000 uANC + 150000000 uusd
// ---
// user2 unlocked uANC    0 + 34500000 = 34500000
// user2 unlocked uusd    0 + 150000000 = 150000000
//
// Step 2. borrow
// to balance 34500000 uANC, needs 842355118 * 34500000 / 138800000 = 209375011 uusd
// user deposits 150000000, needs to borrow 209375011 - 150000000 = 59375011 uusd
// debt units to add = 420000000000000 * 59375011 / 441000000 = 56547629523809
// ---
// total debt units       420000000000000 + 56547629523809 = 476547629523809
// user2 debt units       0 + 56547629523809 = 56547629523809
// user2 unlocked uusd    150000000 + 59375011 = 209375011
// debt                   441000000 + 59375011 = 500375011
//
// Step 3. provide liquidity
// sends 34500000 uANC + 209375011 uusd to primary pool
// mint amount = min(341930857 * 34500000 / 138800000, 341930857 * 209375011 / 842355118) = 84990018 uLP
// ---
// user2 unlocked uANC    34500000 - 34500000 = 0
// user2 unlocked uusd    209375011 - 209375011 = 1
// user2 unlocked uLP     0 + 84990018 = 84990018
// primary pool uANC      138800000 + 34500000 = 173300000
// primary pool uusd      842355118 + 209375011 = 1051730129
// primary pool uLP       341930857 + 84990018 = 426920875
//
// Step 4. bond
// send 84990018 uLP to Astro generator
// contract should receive 1000000 uASTRO + 500000 uANC
// bond units to add = 170235131000000 * 84990018 / 171695726 = 84267018084785
// ---
// total bond units       170235131000000 + 84267018084785 = 254502149084785
// user2 bond units       0 + 84267018084785 = 84267018084785
// user2 unlocked uLP     84990018 - 84990018 = 0
// bond                   171695726 + 84990018 = 256685744
// pending reward uASTRO  1000000 + 1000000 = 2000000
// pending reward uANC    500000 + 500000 = 1000000
//
// Result
// ---
// total bond units       254502149084785
// total debt units       476547629523809
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   256685744
// debt                   500375011
// primary pool uANC      173300000
// primary pool uusd      1051730129
// primary pool uLP       426920875
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 173300000, 1051730129) / 1e6 = 6.034022
// primary value = 173300000 * 6.034022 = 1045696012
// secondary value = 1051730129 * 1 = 1051730129
// pool value = 2 * sqrt(1045696012 * 1051730129) = 2097417460
// total bond value = 2097417460 * 256685744 / 426920875 = 1261070124
// total debt value = 500375011
//
// User1 health:
// bond amount = 256685744 * 170235131000000 / 254502149084785 = 171695726
// bond value = 1261070124 * 170235131000000 / 254502149084785 = 843523084
// debt amount = 500375011 * 420000000000000 / 476547629523809 = 441000000
// debt value = 500375011 * 420000000000000 / 476547629523809 = 441000000
// ltv = 441000000 / 843523084 = 0.522807269136928563
//
// User2 health:
// bond amount = 256685744 * 84267018084785 / 254502149084785 = 84990017
// bond value = 1261070124 * 84267018084785 / 254502149084785 = 417547039
// debt amount = 500375011 * 56547629523809 / 476547629523809 = 59375010
// debt value = 500375011 * 56547629523809 / 476547629523809 = 59375010
// ltv = 59375010 / 417547039 = 0.142199571435590996
//--------------------------------------------------------------------------------------------------

async function testOpenPosition2() {
  process.stdout.write("\n5. Opening position for user 2... ");
  const { txhash } = await sendTransaction(user2, [
    new MsgExecuteContract(user2.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "34500000",
        spender: field,
      },
    }),
    new MsgExecuteContract(
      user2.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                cw20: anchorToken,
              },
              amount: "34500000",
            },
          },
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "150000000",
            },
          },
          {
            borrow: {
              amount: "59375011",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005", // 0.5%
            },
          },
        ],
      },
      {
        uusd: "150000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "256685744",
    debt: "500375011",
    primaryPair: {
      assets: [
        { amount: "1051730129" }, // uusd
        { amount: "173300000" }, // uANC
      ],
      total_share: "426920875",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "254502149084785",
      total_debt_units: "476547629523809",
      pending_rewards: [
        { amount: "2000000" }, // uASTRO
        { amount: "1000000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "170235131000000",
          debt_units: "420000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "171695726",
          bond_value: "843523084",
          debt_amount: "441000000",
          debt_value: "441000000",
          ltv: "0.522807269136928563",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84267018084785",
          debt_units: "56547629523809",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "84990017",
          bond_value: "417547039",
          debt_amount: "59375010",
          debt_value: "59375010",
          ltv: "0.142199571435590996",
        },
      },
    ],
  });

  // Also, make sure the `positions` query works
  const response: PositionsResponse = await terra.wasm.contractQuery(field, {
    positions: {},
  });
  expect(response).to.deep.equal([
    {
      user: user1.key.accAddress,
      position: {
        bond_units: "170235131000000",
        debt_units: "420000000000000",
        unlocked_assets: [],
      },
    },
    {
      user: user2.key.accAddress,
      position: {
        bond_units: "84267018084785",
        debt_units: "56547629523809",
        unlocked_assets: [],
      },
    },
  ]);
}

//--------------------------------------------------------------------------------------------------
// Test 6. Pay Debt
//
// Prior to execution:
// ---
// total bond units       254502149084785
// total debt units       476547629523809
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   256685744
// debt                   500375011
// primary pool uANC      173300000
// primary pool uusd      1051730129
// primary pool uLP       426920875
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// Step 1. receiving user deposit
// user1 deposits 100 UST to contract
// ---
// user1 unlocked uusd    0 + 100000000 = 100000000
//
// Step 2. repay
// repay 100 UST
// debt units to reduce: 420000000000000 * 100000000 / 441000000 = 95238095238095
// ---
// debt                   500375011 - 100000000 = 400375011
// total debt units       476547629523809 - 95238095238095 = 381309534285714
// user1 debt units       420000000000000 - 95238095238095 = 324761904761905
// user1 unlocked uusd    100000000 - 100000000 = 0
//
// Result
// ---
// total bond units       254502149084785
// total debt units       381309534285714
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   256685744
// debt                   400375011
// primary pool uANC      173300000
// primary pool uusd      1051730129
// primary pool uLP       426920875
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = 6.034022 (unchanged)
// total bond value = 1261070124 (unchanged)
// total debt value = 400375011
//
// User1 health:
// bond amount = 256685744 * 170235131000000 / 254502149084785 = 171695726
// bond value = 1261070124 * 170235131000000 / 254502149084785 = 843523084
// debt amount = 400375011 * 324761904761905 / 381309534285714 = 341000000
// debt value = 400375011 * 324761904761905 / 381309534285714 = 341000000
// ltv = 341000000 / 843523084 = 0.404256867972092154
//
// User2 health:
// bond amount = 256685744 * 84267018084785 / 254502149084785 = 84990017
// bond value = 1261070124 * 84267018084785 / 254502149084785 = 417547039
// debt amount = 400375011 * 56547629523809 / 381309534285714 = 59375010
// debt value = 400375011 * 56547629523809 / 381309534285714 = 59375010
// ltv = 59375010 / 417547039 = 0.142199571435590996
//--------------------------------------------------------------------------------------------------

async function testPayDebt() {
  process.stdout.write("\n6. User 1 paying debt... ");
  const { txhash } = await sendTransaction(user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "100000000",
            },
          },
          {
            repay: {
              amount: "100000000",
            },
          },
        ],
      },
      {
        uusd: "100000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "256685744",
    debt: "400375011",
    primaryPair: {
      assets: [
        { amount: "1051730129" }, // uusd
        { amount: "173300000" }, // uANC
      ],
      total_share: "426920875",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "254502149084785",
      total_debt_units: "381309534285714",
      pending_rewards: [
        { amount: "2000000" }, // uASTRO
        { amount: "1000000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "170235131000000",
          debt_units: "324761904761905",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "171695726",
          bond_value: "843523084",
          debt_amount: "341000000",
          debt_value: "341000000",
          ltv: "0.404256867972092154",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84267018084785",
          debt_units: "56547629523809",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "84990017",
          bond_value: "417547039",
          debt_amount: "59375010",
          debt_value: "59375010",
          ltv: "0.142199571435590996",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 7. Reduce Position, Pt. 1
//
// Prior to execution:
// ---
// total bond units       254502149084785
// total debt units       381309534285714
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       170235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   256685744
// debt                   400375011
// primary pool uANC      173300000
// primary pool uusd      1051730129
// primary pool uLP       426920875
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// Step 1. unbond
// user1 has 170235131000000 bond units; we try reduce it by 30000000000000
// amount to unbond: 256685744 * 30000000000000 / 254502149084785 = 30257396
// during unbonding, 1000000 uASTRO + 500000 uANC rewards were automatically claimed
// ---
// bond                   256685744 - 30257396 = 226428348
// total bond units       254502149084785 - 30000000000000 = 224502149084785
// user1 bond units       170235131000000 - 30000000000000 = 140235131000000
// user1 unlocked uLP     0 + 30257396 = 30257396
// pending reward uASTRO  2000000 + 1000000 = 3000000
// pending reward uANC    1000000 + 500000 = 1500000
//
// Step 2. remove liquidity
// burn of of user1's 30257396 uLP
// ANC to be released: 173300000 * 30257396 / 426920875 = 12282385
// UST to be released: 1051730129 * 30257396 / 426920875 = 74539843
// ---
// primary pool uANC      173300000 - 12282385 = 161017615
// primary pool uusd      1051730129 - 74539843 = 977190286
// primary pool uLP       426920875 - 30257396 = 396663479
// user1 unlocked uANC    0 + 12282385 = 12282385
// user1 unlocked uusd    0 + 74539843 = 74539843
// user1 unlocked uLP     30257396 - 30257396 = 0
//
// Step 3. refund
// send all 12282385 uANC + 74539843 uusd to user1
// ---
// user1 unlocked uANC    12282385 - 12282385 = 0
// user1 unlocked uusd    74539843 - 74539843 = 1
//
// Result
// ---
// total bond units       224502149084785
// total debt units       381309534285714
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       140235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   226428348
// debt                   400375011
// primary pool uANC      161017615
// primary pool uusd      977190286
// primary pool uLP       396663479
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 161017615, 977190286) / 1e6 = 6.031382
// primary value = 161017615 * 6.031382 = 971158744
// secondary value = 977190286 * 1 = 977190286
// pool value = 2 * sqrt(971158744 * 977190286) = 1948339692
// total bond value = 1948339692 * 226428348 / 396663479 = 1112175335
// total debt value = 400375011
//
// User1 health:
// bond amount = 226428348 * 140235131000000 / 224502149084785 = 141438329
// bond value = 1112175335 * 140235131000000 / 224502149084785 = 694719647
// debt amount = 400375011 * 324761904761905 / 381309534285714 = 341000000
// debt value = 400375011 * 324761904761905 / 381309534285714 = 341000000
// ltv = 341000000 / 694719647 = 00.490845482019310157
//
// User2 health:
// bond amount = 226428348 * 84267018084785 / 224502149084785 = 84990018
// bond value = 1112175335 * 84267018084785 / 224502149084785 = 417455687
// debt amount = 400375011 * 56547629523809 / 381309534285714 = 59375010
// debt value = 400375011 * 56547629523809 / 381309534285714 = 59375010
// ltv = 59375010 / 417455687 = 0.14223068902640198(0)
//--------------------------------------------------------------------------------------------------

async function testReducePosition1() {
  process.stdout.write("\n7. User 1 reducing position... ");
  const { txhash } = await sendTransaction(user1, [
    new MsgExecuteContract(user1.key.accAddress, field, {
      update_position: [
        {
          unbond: {
            bond_units_to_reduce: "30000000000000",
          },
        },
      ],
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "226428348",
    debt: "400375011",
    primaryPair: {
      assets: [
        { amount: "977190286" }, // uusd
        { amount: "161017615" }, // uANC
      ],
      total_share: "396663479",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "224502149084785",
      total_debt_units: "381309534285714",
      pending_rewards: [
        { amount: "3000000" }, // uASTRO
        { amount: "1500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "140235131000000",
          debt_units: "324761904761905",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "141438329",
          bond_value: "694719647",
          debt_amount: "341000000",
          debt_value: "341000000",
          ltv: "0.490845482019310157",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84267018084785",
          debt_units: "56547629523809",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "84990018",
          bond_value: "417455687",
          debt_amount: "59375010",
          debt_value: "59375010",
          ltv: "0.14223068902640198",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 8. Dump
//
// Prior to execution:
// ---
// total bond units       224502149084785
// total debt units       381309534285714
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       140235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   226428348
// debt                   400375011
// primary pool uANC      161017615
// primary pool uusd      977190286
// primary pool uLP       396663479
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// We dump 100 ANC token in the AMM, which should barely make user1 liquidatable
// UST return amount = computeXykSwapOutput(100000000, 161017615, 977190286) = 373254010 uusd
// ---
// primary pool uANC      161017615 + 100000000 = 261017615
// primary pool uusd      977190286 - 373254010 = 603936276
//
// Result
// ---
// total bond units       224502149084785
// total debt units       381309534285714
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       140235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   226428348
// debt                   400375011
// primary pool uANC      261017615
// primary pool uusd      603936276
// primary pool uLP       396663479
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 261017615, 603936276) / 1e6 = 2.304945
// primary value = 261017615 * 2.304945 = 601631246
// secondary value = 603936276 * 1 = 603936276
// pool value = 2 * sqrt(601631246 * 603936276) = 1205565318
// total bond value = 1205565318 * 226428348 / 396663479 = 688175690
// total debt value = 400375011
//
// User1 health:
// bond amount = 226428348 * 140235131000000 / 224502149084785 = 141438329
// bond value = 688175690 * 140235131000000 / 224502149084785 = 429868526
// debt amount = 400375011 * 324761904761905 / 381309534285714 = 341000000
// debt value = 400375011 * 324761904761905 / 381309534285714 = 341000000
// ltv = 341000000 / 429868526 = 0.793265799599387278
//
// User2 health:
// bond amount = 226428348 * 84267018084785 / 224502149084785 = 84990018
// bond value = 688175690 * 84267018084785 / 224502149084785 = 258307163
// debt amount = 400375011 * 56547629523809 / 381309534285714 = 59375010
// debt value = 400375011 * 56547629523809 / 381309534285714 = 59375010
// ltv = 59375010 / 258307163 = 0.229862034449273092
//--------------------------------------------------------------------------------------------------

async function testDump() {
  process.stdout.write("\n8. Dumping ANC to crash the price... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      send: {
        amount: "100000000",
        contract: ancUstPair,
        msg: encodeBase64({
          swap: {
            max_spread: "0.5",
          },
        }),
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "226428348",
    debt: "400375011",
    primaryPair: {
      assets: [
        { amount: "603936276" }, // uusd
        { amount: "261017615" }, // uANC
      ],
      total_share: "396663479",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "224502149084785",
      total_debt_units: "381309534285714",
      pending_rewards: [
        { amount: "3000000" }, // uASTRO
        { amount: "1500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "140235131000000",
          debt_units: "324761904761905",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "141438329",
          bond_value: "429868526",
          debt_amount: "341000000",
          debt_value: "341000000",
          ltv: "0.793265799599387278",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84267018084785",
          debt_units: "56547629523809",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "84990018",
          bond_value: "258307163",
          debt_amount: "59375010",
          debt_value: "59375010",
          ltv: "0.229862034449273092",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 9. Liquidation
//
// Prior to execution
// ---
// total bond units       224502149084785
// total debt units       381309534285714
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       140235131000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   226428348
// debt                   400375011
// primary pool uANC      261017615
// primary pool uusd      603936276
// primary pool uLP       396663479
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// Step 1. unbond
// reduce all of user1's 140235131000000 bond units
// amount to unbond: 226428348 * 140235131000000 / 224502149084785 = 141438329
// upon unbonding, we automatically claim 1000000 uASTRO + 500000 uANC rewards
// ---
// bond                   226428348 - 141438329 = 84990019
// total bond units       224502149084785 - 140235131000000 = 84267018084785
// user1 bond units       140235131000000 - 140235131000000 = 0
// user1 unlocked uLP     0 + 141095037 = 141095037
// pending reward uASTRO  3000000 + 1000000 = 4000000
// pending reward uANC    1500000 + 500000 = 2000000
//
// Step 2. remove liquidity
// burn all of user1's 141438329 uLP
// ANC to be released: 261017615 * 141438329 / 396663479 = 93071072
// UST to be released: 603936276 * 141438329 / 396663479 = 215345607
// ---
// primary pool uANC      261017615 - 93071072 = 167946543
// primary pool uusd      603936276 - 215345607 = 388590669
// primary pool uLP       396663479 - 141438329 = 255225150
// user1 unlocked uANC    0 + 93071072 = 93071072
// user1 unlocked uusd    0 + 215345607 = 215345607
// user1 unlocked uLP     141438329 - 141438329 = 0
//
// Step 3. cover
// UST needed amount = 341000000 - 215345607 = 125654393
// to account for numerical issue, we increment UST needed amount by 1:  125654394
// ANC sell amount = computeXykSwapInput(125654394, 167946543, 388590669) = 80617261
// UST actual return amount = computeXykSwapOutput(80617261, 167946543, 388590669) = 125654393
// ---
// primary pool uANC      167946543 + 80617261 = 248563804
// primary pool uusd      388590669 - 125654393 = 262936276
// user1 unlocked uANC    93071072 - 80617261 = 12453811
// user1 unlocked uusd    215345607 + 125654393 = 341000000
//
// Step 4. repay
// user1's debt amount: 341000000. repay this amount
// ---
// debt                   400375011 - 341000000 = 59375011
// total debt units       381309534285714 - 324761904761905 = 56547629523809
// user1 debt units       324761904761905 - 324761904761905 = 0
// user1 unlocked uusd    341000000 - 341000000 = 0
//
// Step 5. refund the liquidator
// ANC bonus amount = 12453811 * 0.05 = 622690
// ---
// user1 unlocked uANC    12453811 - 622690 = 11831121
//
// Step 6. refund the user
// refund all the remaining unlocked ANC to user
// ---
// user1 unlocked uANC    11831121 - 11831121 = 0
//
// Result
// ---
// total bond units       84267018084785
// total debt units       56547629523809
// pending reward uASTRO  4000000
// pending reward uANC    2000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   84990019
// debt                   59375011
// primary pool uANC      248563804
// primary pool uusd      262936276
// primary pool uLP       255225150
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 248563804, 262936276) / 1e6 = 1.053583
// primary value = 248563804 * 1.053583 = 261882598
// secondary value = 262936276 * 1 = 262936276
// pool value = 2 * sqrt(261882598 * 262936276) = 524817816
// total bond value = 524817816 * 84990019 / 255225150 = 174764423
// total debt value = 59375011
//
// User1 health:
// bondValue = 0
// debtValue = 0
// ltv = null
//
// User2 health:
// bond and debt values are the same as the state's as user2 is the only user now
// ltv = 59375011 / 174764423 = 0.339743123805009215
//--------------------------------------------------------------------------------------------------

async function testLiquidation() {
  process.stdout.write("\n9. Liquidation user 1... ");
  const { txhash } = await sendTransaction(liquidator, [
    new MsgExecuteContract(liquidator.key.accAddress, field, {
      liquidate: {
        user: user1.key.accAddress,
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "84990019",
    debt: "59375011",
    primaryPair: {
      assets: [
        { amount: "262936276" }, // uusd
        { amount: "248563804" }, // uANC
      ],
      total_share: "255225150",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "84267018084785",
      total_debt_units: "56547629523809",
      pending_rewards: [
        { amount: "4000000" }, // uASTRO
        { amount: "2000000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "0",
          bond_value: "0",
          debt_amount: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84267018084785",
          debt_units: "56547629523809",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "84990019",
          bond_value: "174764423",
          debt_amount: "59375011",
          debt_value: "59375011",
          ltv: "0.339743123805009215",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 11. Reduce Position, Pt. 2
//
// Prior to execution:
// ---
// total bond units       84267018084785
// total debt units       56547629523809
// pending reward uASTRO  4000000
// pending reward uANC    2000000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       84267018084785
// user2 debt units       56547629523809
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   84990019
// debt                   59375011
// primary pool uANC      248563804
// primary pool uusd      262936276
// primary pool uLP       255225150
// astro pool uASTRO      101600000
// astro pool uusd        147644882
// astro pool uLP         122474487
//
// Step 1. unbond
// unbond all of user2's 84990019 uLP
// contract receives 1000000 uASTRO + 500000 uANC staking reward
// ---
// bond                   0
// total bond units       0
// user2 bond units       0
// user2 unlocked uLP     0 + 84990019 = 84990019
// pending reward uASTRO  4000000 + 1000000 = 5000000
// pending reward uANC    2000000 + 500000 = 2500000
//
// Step 2. remove liquidity
// burn all of user2's 84990019 uLP
// ANC to be released: 248563804 * 84990019 / 255225150 = 82771789
// UST to be released: 262936276 * 84990019 / 255225150 = 87557825
// ---
// primary pool uANC      248563804 - 82771789 = 165792015
// primary pool uusd      262936276 - 87557825 = 175378451
// primary pool uLP       255225150 - 84990019 = 170235131
// user2 unlocked uANC    0 + 82771789 = 82771789
// user2 unlocked uusd    1 + 87557825 = 87557825
// user2 unlocked uLP     0
//
// Step 3. repay
// user2's remaining debts: 59375011 uusd
// ---
// debt                   0
// total debt units       0
// user2 debt units       0
// user2 unlocked uusd    87557825 - 59375011 = 28182814
//
// Step 5. refund
// send all ANC and uusd to user2
// ---
// user2 unlocked uANC    0
// user2 unlocked uusd    0
//
// Result
// ---
// total bond units       0
// total debt units       0
// pending reward uASTRO  5000000
// pending reward uANC    2500000
// pending reward uusd    0
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       0
// user2 debt units       0
// user2 unlocked uANC    0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   0
// debt                   0
// primary pool uANC      165792015
// primary pool uusd      175378451
// primary pool uLP       170235131
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//--------------------------------------------------------------------------------------------------

async function testReducePosition2() {
  process.stdout.write("\n10. User 2 closhing position... ");
  const { txhash } = await sendTransaction(user2, [
    new MsgExecuteContract(user2.key.accAddress, field, {
      update_position: [
        {
          unbond: {
            bond_units_to_reduce: "84267018084785",
          },
        },
        {
          repay: {
            amount: "60000000",
          },
        },
      ],
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "0",
    debt: "0",
    primaryPair: {
      assets: [
        { amount: "175378451" }, // uusd
        { amount: "165792015" }, // uANC
      ],
      total_share: "170235131",
    },
    astroPair: {
      assets: [
        { amount: "147644882" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "0",
      total_debt_units: "0",
      pending_rewards: [
        { amount: "5000000" }, // uASTRO
        { amount: "2500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "0",
          bond_value: "0",
          debt_amount: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [],
        },
        health: {
          bond_amount: "0",
          bond_value: "0",
          debt_amount: "0",
          debt_value: "0",
          ltv: null,
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nInfo"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(treasury.key.accAddress)} as treasury`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);
  console.log(`Use ${chalk.cyan(liquidator.key.accAddress)} as liquidator`);

  console.log(chalk.yellow("\nSetup"));

  await setupTest();

  console.log(chalk.yellow("\nTests"));

  await testConfig();
  await testOpenPosition1();
  await testHarvest();
  await testAccrueInterest();
  await testOpenPosition2();
  await testPayDebt();
  await testReducePosition1();
  await testDump();
  await testLiquidation();
  await testReducePosition2();

  console.log(chalk.green("\nAll tests successfully completed!\n"));
})();
