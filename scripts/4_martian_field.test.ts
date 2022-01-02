import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, encodeBase64 } from "./helpers";
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
import { Config } from "./types";

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
  let { cw20CodeId, address } = await deployCw20Token(
    terra,
    deployer,
    undefined,
    "Anchor Token",
    "ANC"
  );
  anchorToken = address;

  ({ address } = await deployCw20Token(terra, deployer, cw20CodeId, "Astroport Token", "ASTRO"));
  astroToken = address;

  ({ astroportFactory } = await deployAstroportFactory(terra, deployer, cw20CodeId));

  let { astroportPair, astroportLpToken } = await deployAstroportPair(
    terra,
    deployer,
    astroportFactory,
    anchorToken
  );
  ancUstPair = astroportPair;
  ancUstLpToken = astroportLpToken;

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(
    terra,
    deployer,
    astroportFactory,
    astroToken
  ));
  astroUstPair = astroportPair;
  astroUstLpToken = astroportLpToken;

  ({ astroGenerator } = await deployAstroGenerator(
    terra,
    deployer,
    ancUstLpToken,
    astroToken,
    anchorToken
  ));

  ({ oracle } = await deployOracle(terra, deployer));

  ({ bank } = await deployRedBank(terra, deployer));

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
    max_ltv: "0.75", // 75%, i.e. for every 100 UST asset there must be no more than 75 UST debt
    fee_rate: "0.2", // 20%
    bonus_rate: "0.05", // 5%
  };

  ({ field } = await deployMartianField(terra, deployer, config));

  process.stdout.write("Configuring ANC and UST price oracle...");
  await sendTransaction(terra, deployer, [
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
  await sendTransaction(terra, deployer, [
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
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Astro generator contract with ANC and ASTRO... ");
  await sendTransaction(terra, deployer, [
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
  await sendTransaction(terra, deployer, [
    new MsgSend(deployer.key.accAddress, bank, { uusd: 100000000000 }),
  ]);
  console.log(chalk.green("Done!"));

  // deployer provides 69 ANC + 420 UST
  // should receive sqrt(69000000 * 420000000) = 170235131 uLP
  process.stdout.write("Provide initial liquidity to ANC-UST Pair... ");
  await sendTransaction(terra, deployer, [
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
  await sendTransaction(terra, deployer, [
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
    ancUstPool: {
      assets: [
        { amount: "420000000" }, // uusd
        { amount: "69000000" },  // uANC
      ],
      total_share: "170235131",
    },
    astroUstPool: {
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
// attempts to borrow 420000000 uusd; receives deductTax(420000000) = 419580419 uusd
// ---
// total debt units       0 + 420000000000000 = 420000000000000
// user1 debt units       0 + 420000000000000 = 420000000000000
// user1 unlocked uusd    0 + 419580419 = 419580419
// debt                   0 + 420000000 = 420000000
//
// Step 3. provide liquidity
// sends 69000000 uANC + deductTax(419580419) = 419161257 uusd to primary pool
// total tx cost is addTax(419161257) = 419580418 uusd
// mint amount = min(170235131 * 69000000 / 69000000, 170235131 * 419161257 / 420000000) = 169895170 uLP
// ---
// user1 unlocked uANC    69000000 - 69000000 = 0
// user1 unlocked uusd    419580419 - 419580418 = 1
// user1 unlocked uLP     0 + 169895170 = 169895170
// primary pool uANC      69000000 + 69000000 = 138000000
// primary pool uusd      420000000 + 419161257 = 839161257
// primary pool uLP       170235131 + 169895170 = 340130301
//
// Step 4. bond
// send 169895170 uLP to Astro generator
// contract should receive 1000000 uASTRO + 500000 uANC
// ---
// total bond units       0 + 169895170000000 = 169895170000000
// user1 bond units       0 + 169895170000000 = 169895170000000
// user1 unlocked uLP     169895170 - 169895170 = 0
// bond                   0 + 169895170 = 169895170
// pending reward uASTRO  0 + 1000000 = 1000000
// pending reward uANC    0 + 500000 = 500000
//
// Result
// ---
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   169895170
// debt                   420000000
// primary pool uANC      138000000
// primary pool uusd      839161257
// primary pool uLP       340130301
// astro pool uASTRO      100000000
// astro pool uusd        150000000
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 138000000, 839161257) / 1000000 = 6.037131
// primary value = 138000000 * 6.037131 = 833124078
// secondary value = 839161257 * 1 = 839161257
// pool value = 2 * sqrt(833124078 * 839161257) = 1672274436
// total bond value = 1672274436 * 169895170 / 340130301 = 835301496
// total debt value = 420000000
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 420000000 / 835301496 = 0.502812459945600288
//--------------------------------------------------------------------------------------------------

async function testOpenPosition1() {
  process.stdout.write("\n2. Opening position for user 1... ");
  const { txhash } = await sendTransaction(terra, user1, [
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
    bond: "169895170",
    debt: "420000000",
    ancUstPool: {
      assets: [
        { amount: "839161257" }, // uusd
        { amount: "138000000" }, // uANC
      ],
      total_share: "340130301",
    },
    astroUstPool: {
      assets: [
        { amount: "150000000" }, // uusd
        { amount: "100000000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1000000" }, // uASTRO
        { amount: "500000" },  // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "835301496",
          debt_value: "420000000",
          ltv: "0.502812459945600288",
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
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   169895170
// debt                   420000000
// primary pool uANC      138000000
// primary pool uusd      839161257
// primary pool uLP       340130301
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
// UST fee after tax = deductTax(585884) = 585298 uusd
// tranfer cost = addTax(585298) = 585883
// ---
// pending reward uASTRO  2000000 - 400000 = 1600000
// pending reward uANC    1000000 - 200000 = 800000
//
// Step 3. swap ASTRO >> UST
// return amount = computeXykSwapOutput(1600000, 100000000, 150000000) = 2355118 uusd
// return amount after tax = deductTax(2355118) = 2352765 uusd
// transfer cost = addTax(2352765) = 2355117 uusd
// ---
// pending reward uASTRO  1600000 - 1600000 = 0
// pending reward uusd    0 + 2352765 = 2352765
// astro pool uASTRO      100000000 + 1600000 = 101600000
// astro pool uusd        150000000 - 2355117 = 147644883
//
// Step 4. balance
// ANC price = computeXykSwapOutput(1000000, 138000000, 839161257) / 1e6 = 6.037131
// ANC value = 800000 * 6.037131 = 4829704
// UST value = 2352765 * 1 = 2352765
// value diff = 4829704 - 2352765 = 2476939
// value to swap = 2476939 / 2 = 1238469
// amount to swap = 800000 * 1238469 / 4829704 = 205142
// UST return amount = computeXykSwapOutput(205142, 138000000, 839161257) = 1241855
// UST return amount after tax = deductTax(1241855) = 1240614
// transfer cost = addTax(1240614) = 1241854
// ---
// pending reward uANC    800000 - 205142 = 594858
// pending reward uusd    2352765 + 1240614 = 3593379
// primary pool uANC      138000000 + 205142 = 138205142
// primary pool uusd      839161257 - 1241854 = 837919403
//
// Step 2. provide liquidity
// sends 594858 uANC + deductTax(3593379) = 3589789 uusd to pool
// transfer cost = addTax(3589789) = 3593378 uusd
// shares minted = min(340130301 * 594858 / 138205142, 340130301 * 3589789 / 837919403) = 1457175 uLP
// ---
// pending reward uANC    594858 - 594858 = 0
// pending reward uusd    3593379 - 3593378 = 1
// pending reward uLP     0 + 1457175 = 1457175
// primary pool uANC      138205142 + 594858 = 138800000
// primary pool uusd      837919403 + 3589789 = 841509192
// primary pool uLP       340130301 + 1457175 = 341587476
//
// Step 4. bond
// send 1457175 uLP to staking contract
// bond units should not change in a harvest transaction
// when we bond, we receive another 1000000 uASTRO + 500000 uANC
// this is not how the actual Astro generator will behave (since we already claimed rewards in the
// same tx) but still, our contract should account for this propoerly
// ---
// pending reward uLP     1457175 - 1457175 = 0
// bond                   169895170 + 1457175 = 171352345
// pending reward uASTRO  0 + 1000000 = 1000000
// pending reward uANC    0 + 500000 = 500000
//
// Result
// ---
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   171352345
// debt                   420000000
// primary pool uANC      138800000
// primary pool uusd      841509192
// primary pool uLP       341587476
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 138800000, 841509192) / 1000000 = 6.019379
// primary value = 138800000 * 6.019379 = 835489805
// secondary value = 841509192 * 1 = 841509192
// pool value = 2 * sqrt(835489805 * 841509192) = 1676988194
// total bond value = 1676988194 * 171352345 / 341587476 = 841236519
// total debt value = 420000000
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 420000000 / 841236519 = 0.499265058653498612
//--------------------------------------------------------------------------------------------------

async function testHarvest() {
  process.stdout.write("\n3. Harvesting... ");
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, field, {
      harvest: {
        max_spread: "0.02", // if not specified, Astroport applied a default max spread of 0.5%
        slippage_tolerance: undefined,
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "171352345",
    debt: "420000000",
    ancUstPool: {
      assets: [
        { amount: "841509192" }, // uusd
        { amount: "138800000" }, // uANC
      ],
      total_share: "341587476",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "1000000" }, // uASTRO
        { amount: "500000" },  // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "841236519",
          debt_value: "420000000",
          ltv: "0.499265058653498612",
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
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   171352345
// debt                   420000000
// primary pool uANC      138800000
// primary pool uusd      841509192
// primary pool uLP       341587476
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// We forcibly set the strategy's debt to 441000000 to simulate accrual of a 5% interest
//
// Result
// ---
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   171352345
// debt                   441000000
// primary pool uANC      138800000
// primary pool uusd      841509192
// primary pool uLP       341587476
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = 6.019379 (unchanged)
// total bond value = 841236519 (unchanged)
// total debt value = 441000000 uusd
//
// User1 health:
// same as state as user1 is the only user now
// ltv = 441000000 / 841236519 = 0.524228311586173543
//--------------------------------------------------------------------------------------------------

async function testAccrueInterest() {
  process.stdout.write("\n4. Accruing interest... ");
  const { txhash } = await sendTransaction(terra, deployer, [
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
    bond: "171352345",
    debt: "441000000",
    ancUstPool: {
      assets: [
        { amount: "841509192" }, // uusd
        { amount: "138800000" }, // uANC
      ],
      total_share: "341587476",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "1000000" }, // uASTRO
        { amount: "500000" },  // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "841236519",
          debt_value: "441000000",
          ltv: "0.524228311586173543",
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
// total bond units       169895170000000
// total debt units       420000000000000
// pending reward uASTRO  1000000
// pending reward uANC    500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// bond                   171352345
// debt                   441000000
// primary pool uANC      138800000
// primary pool uusd      841509192
// primary pool uLP       341587476
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// Step 1. deposit
// contract receives 34500000 uANC + 150000000 uusd
// ---
// user2 unlocked uANC    0 + 34500000 = 34500000
// user2 unlocked uusd    0 + 150000000 = 150000000
//
// Step 2. borrow
// to balance 34500000 uANC, needs 841509192 * 34500000 / 138800000 = 209164748 uusd
// user deposits 150000000, needs to borrow 209164748 - 150000000 = 59164748 uusd
// attempts to borrow 59164748 uusd; should receive deductTax(59164748) = 59105642 uusd
// debt units to add = 420000000000000 * 59164748 / 441000000 = 56347379047619
// ---
// total debt units       420000000000000 + 56347379047619 = 476347379047619
// user2 debt units       0 + 56347379047619 = 56347379047619
// user2 unlocked uusd    150000000 + 59105642 = 209105642
// debt                   441000000 + 59164748 = 500164748
//
// Step 3. provide liquidity
// sends 34500000 uANC + deductTax(209105642) = 208896745 uusd to primary pool
// transfer cost = addTax(208896745) = 209105641 uusd
// mint amount = min(341587476 * 34500000 / 138800000, 341587476 * 208896745 / 841509192) = 84795879 uLP
// ---
// user2 unlocked uANC    34500000 - 34500000 = 0
// user2 unlocked uusd    209105642 - 209105641 = 1
// user2 unlocked uLP     0 + 84795879 = 84795879
// primary pool uANC      138800000 + 34500000 = 173300000
// primary pool uusd      841509192 + 208896745 = 1050405937
// primary pool uLP       341587476 + 84795879 = 426383355
//
// Step 4. bond
// send 84795879 uLP to Astro generator
// contract should receive 1000000 uASTRO + 500000 uANC
// bond units to add = 169895170000000 * 84795879 / 171352345 = 84074777488481
// ---
// total bond units       169895170000000 + 84074777488481 = 253969947488481
// user2 bond units       0 + 84074777488481 = 84074777488481
// user2 unlocked uLP     84795879 - 84795879 = 0
// bond                   171352345 + 84795879 = 256148224
// pending reward uASTRO  1000000 + 1000000 = 2000000
// pending reward uANC    500000 + 500000 = 1000000
//
// Result
// ---
// total bond units       253969947488481
// total debt units       476347379047619
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   256148224
// debt                   500164748
// primary pool uANC      173300000
// primary pool uusd      1050405937
// primary pool uLP       426383355
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 173300000, 1050405937) / 1e6 = 6.026425
// primary value = 173300000 * 6.026425 = 1044379452
// secondary value = 1050405937 * 1 = 1050405937
// pool value = 2 * sqrt(1044379452 * 1050405937) = 2094776720
// total bond value = 2094776720 * 256148224 / 426383355 = 1258429369
// total debt value = 500164748
//
// User1 health:
// bond value = 1258429369 * 169895170000000 / 253969947488481 = 841836105
// debt value = 500164748 * 420000000000000 / 476347379047619 = 441000000
// ltv = 441000000 / 841836105 = 0.523854937298038553
//
// User2 health:
// bond value = 1258429369 * 84074777488481 / 253969947488481 = 416593263
// debt value = 500164748 * 56347379047619 / 476347379047619 = 59164747
// ltv = 59164747 / 416593263 = 0.142020412365621956
//--------------------------------------------------------------------------------------------------

async function testOpenPosition2() {
  process.stdout.write("\n5. Opening position for user 2... ");
  const { txhash } = await sendTransaction(terra, user2, [
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
              amount: "59164748",
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
    bond: "256148224",
    debt: "500164748",
    ancUstPool: {
      assets: [
        { amount: "1050405937" }, // uusd
        { amount: "173300000" }, // uANC
      ],
      total_share: "426383355",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "253969947488481",
      total_debt_units: "476347379047619",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "2000000" }, // uASTRO
        { amount: "1000000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "841836105",
          debt_value: "441000000",
          ltv: "0.523854937298038553",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84074777488481",
          debt_units: "56347379047619",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "416593263",
          debt_value: "59164747",
          ltv: "0.142020412365621956",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 6. Pay Debt
//
// Prior to execution:
// ---
// total bond units       253969947488481
// total debt units       476347379047619
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       420000000000000
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   256148224
// debt                   500164748
// primary pool uANC      173300000
// primary pool uusd      1050405937
// primary pool uLP       426383355
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// Step 1. receiving user deposit
// user1 deposits 100.1 UST to contract
// ---
// user1 unlocked uusd  1 + 100100000 = 100100001
//
// Step 2. repay
// repay 100 UST
// transaction cost: addTax(100000000) = 100100000
// debt units to reduce: 420000000000000 * 100000000 / 441000000 = 95238095238095
// ---
// debt                 500164748 - 100000000 = 400164748
// total debt units     476347379047619 - 95238095238095 = 381109283809524
// user1 debt units     420000000000000 - 95238095238095 = 324761904761905
// user1 unlocked uusd  100100001 - 100100000 = 1
//
// Result
// ---
// total bond units       253969947488481
// total debt units       381109283809524
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   256148224
// debt                   400164748
// primary pool uANC      173300000
// primary pool uusd      1050405937
// primary pool uLP       426383355
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = 6.026425 (unchanged)
// total bond value = 1258429369 (unchanged)
// total debt value = 400164748
//
// User1 health:
// bond value = 1258429369 * 169895170000000 / 253969947488481 = 841836105
// debt value = 400164748 * 324761904761905 / 381109283809524 = 341000000
// ltv = 341000000 / 841836105 = 0.405066969656759969
//
// User2 health:
// bond value = 1258429369 * 84074777488481 / 253969947488481 = 416593263
// debt value = 400164748 * 56347379047619 / 381109283809524 = 59164747
// ltv = 59164747 / 416593263 = 0.142020412365621956
//--------------------------------------------------------------------------------------------------

async function testPayDebt() {
  process.stdout.write("\n6. User 1 paying debt... ");
  const { txhash } = await sendTransaction(terra, user1, [
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
              amount: "100500000", // we need to deposit slightly more than 100 UST to cover tax
            },                     // the excess deposit will be refunded in the end
          },
          {
            repay: {
              amount: "100000000",
            },
          },
        ],
      },
      {
        uusd: "100500000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "256148224",
    debt: "400164748",
    ancUstPool: {
      assets: [
        { amount: "1050405937" }, // uusd
        { amount: "173300000" },  // uANC
      ],
      total_share: "426383355",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "253969947488481",
      total_debt_units: "381109283809524",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "2000000" }, // uASTRO
        { amount: "1000000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "841836105",
          debt_value: "341000000",
          ltv: "0.405066969656759969",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84074777488481",
          debt_units: "56347379047619",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "416593263",
          debt_value: "59164747",
          ltv: "0.142020412365621956",
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
// total bond units       253969947488481
// total debt units       381109283809524
// pending reward uASTRO  2000000
// pending reward uANC    1000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       169895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   256148224
// debt                   400164748
// primary pool uANC      173300000
// primary pool uusd      1050405937
// primary pool uLP       426383355
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// Step 1. unbond
// user1 has 169895170000000 bond units; we try reduce it by 30000000000000
// amount to unbond: 256148224 * 30000000000000 / 253969947488481 = 30257307
// during unbonding, 1000000 uASTRO + 500000 uANC rewards were automatically claimed
// ---
// bond                   256148224 - 30257307 = 225890917
// total bond units       253969947488481 - 30000000000000 = 223969947488481
// user1 bond units       169895170000000 - 30000000000000 = 139895170000000
// user1 unlocked uLP     0 + 30257307 = 30257307
// pending reward uASTRO  2000000 + 1000000 = 3000000
// pending reward uANC    1000000 + 500000 = 1500000
//
// Step 2. remove liquidity
// burn of of user1's 30257307 uLP
// ANC to be released: 173300000 * 30257307 / 426383355 = 12297833
// UST to be released: 1050405937 * 30257307 / 426383355 = 74539623
// UST to receive: deductTax(74539623) = 74465157
// transaction cost for pool: addTax(74465157) = 74539622
// ---
// primary pool uANC      173300000 - 12297833 = 161002167
// primary pool uusd      1050405937 - 74539622 = 975866315
// primary pool uLP       426383355 - 30257307 = 396126048
// user1 unlocked uANC    0 + 12297833 = 12297833
// user1 unlocked uusd    1 + 74465157 = 74465158
// user1 unlocked uLP     30257307 - 30257307 = 0
//
// Step 3. refund
// send all 12297833 uANC to user1
// UST to send: deductTax(74465158) = 74390767
// transaction cost: addTax(74390767) = 74465157
// ---
// user1 unlocked uANC    12297833 - 12297833 = 0
// user1 unlocked uusd    74465158 - 74465157 = 1
//
// Result
// ---
// total bond units       223969947488481
// total debt units       381109283809524
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       139895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   225890917
// debt                   400164748
// primary pool uANC      161002167
// primary pool uusd      975866315
// primary pool uLP       396126048
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 161002167, 975866315) / 1e6 = 6.023785
// primary value = 161002167 * 6.023785 = 969842438
// secondary value = 975866315 * 1 = 975866315
// pool value = 2 * sqrt(969842438 * 975866315) = 1945699428
// total bond value = 1945699428 * 225890917 / 396126048 = 1109535286
// total debt value = 400164748
//
// User1 health:
// bond value = 1109535286 * 139895170000000 / 223969947488481 = 693033280
// debt value = 400164748 * 324761904761905 / 381109283809524 = 341000000
// ltv = 341000000 / 693033280 = 0.492039862789850438
//
// User2 health:
// bond value = 1109535286 * 84074777488481 / 223969947488481 = 416502005
// debt value = 400164748 * 56347379047619 / 381109283809524 = 59164747
// ltv = 59164747 / 416502005 = 0.142051529859982306
//--------------------------------------------------------------------------------------------------

async function testReducePosition1() {
  process.stdout.write("\n7. User 1 reducing position... ");
  const { txhash } = await sendTransaction(terra, user1, [
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
    bond: "225890917",
    debt: "400164748",
    ancUstPool: {
      assets: [
        { amount: "975866315" }, // uusd
        { amount: "161002167" }, // uANC
      ],
      total_share: "396126048",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "223969947488481",
      total_debt_units: "381109283809524",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "3000000" }, // uASTRO
        { amount: "1500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "693033280",
          debt_value: "341000000",
          ltv: "0.492039862789850438",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84074777488481",
          debt_units: "56347379047619",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "416502005",
          debt_value: "59164747",
          ltv: "0.142051529859982306",
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
// total bond units       223969947488481
// total debt units       381109283809524
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       139895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   225890917
// debt                   400164748
// primary pool uANC      161002167
// primary pool uusd      975866315
// primary pool uLP       396126048
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// We dump 100 ANC token in the AMM, which should barely make user1 liquidatable
// UST return amount = computeXykSwapOutput(100000000, 161002167, 975866315) = 372770359 uusd
// UST return amount after tax = deductTax(372770359) = 372397961
// transfer cost = addTax(372397961) = 372770358
// ---
// primary pool uANC      161002167 + 100000000 = 261002167
// primary pool uusd      975866315 - 372770358 = 603095957
//
// Result
// ---
// total bond units       223969947488481
// total debt units       381109283809524
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       139895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   225890917
// debt                   400164748
// primary pool uANC      261002167
// primary pool uusd      603095957
// primary pool uLP       396126048
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 261002167, 603095957) / 1e6 = 2.301873
// primary value = 261002167 * 2.301873 = 600793841
// secondary value = 603095957 * 1 = 603095957
// pool value = 2 * sqrt(600793841 * 603095957) = 1203887596
// total bond value = 1203887596 * 225890917 / 396126048 = 686517017
// total debt value = 400164748
//
// User1 health:
// bond value = 686517017 * 139895170000000 / 223969947488481 = 428809382
// debt value = 400164748 * 324761904761905 / 381109283809524 = 341000000
// ltv = 341000000 / 428809382 = 0.795225138054465422
//
// User2 health:
// bond value = 686517017 * 84074777488481 / 223969947488481 = 257707634
// debt value = 400164748 * 56347379047619 / 381109283809524 = 59164747
// ltv = 59164747 / 257707634 = 0.229580886222408141
//--------------------------------------------------------------------------------------------------

async function testDump() {
  process.stdout.write("\n8. Dumping ANC to crash the price... ");
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      send: {
        amount: "100000000",
        contract: ancUstPair,
        msg: encodeBase64({
          swap: {
            max_spread: "0.5"
          },
        }),
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "225890917",
    debt: "400164748",
    ancUstPool: {
      assets: [
        { amount: "603095957" }, // uusd
        { amount: "261002167" }, // uANC
      ],
      total_share: "396126048",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "223969947488481",
      total_debt_units: "381109283809524",
      pending_rewards: [
        { amount: "1" },       // uusd
        { amount: "3000000" }, // uASTRO
        { amount: "1500000" }, // uANC
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "428809382",
          debt_value: "341000000",
          ltv: "0.795225138054465422",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84074777488481",
          debt_units: "56347379047619",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "257707634",
          debt_value: "59164747",
          ltv: "0.229580886222408141",
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
// total bond units       223969947488481
// total debt units       381109283809524
// pending reward uASTRO  3000000
// pending reward uANC    1500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       139895170000000
// user1 debt units       324761904761905
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   225890917
// debt                   400164748
// primary pool uANC      261002167
// primary pool uusd      603095957
// primary pool uLP       396126048
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// Step 1. unbond
// reduce all of user1's 139895170000000 bond units
// amount to unbond: 225890917 * 139895170000000 / 223969947488481 = 141095037
// upon unbonding, we automatically claim 1000000 uASTRO + 500000 uANC rewards
// ---
// bond                   225890917 - 141095037 = 84795880
// total bond units       223969947488481 - 139895170000000 = 84074777488481
// user1 bond units       139895170000000 - 139895170000000 = 0
// user1 unlocked uLP     0 + 141095037 = 141095037
// pending reward uASTRO  3000000 + 1000000 = 4000000
// pending reward uANC    1500000 + 500000 = 2000000
//
// Step 2. remove liquidity
// burn of of user1's 141095037 uLP
// ANC to be released: 261002167 * 141095037 / 396126048 = 92965637
// UST to be released: 603095957 * 141095037 / 396126048 = 214815074
// UST to receive: deductTax(214815074) = 214600473
// transaction cost for pool: addTax(214600473) = 214815073
// ---
// primary pool uANC      261002167 - 92965637 = 168036530
// primary pool uusd      603095957 - 214815073 = 388280884
// primary pool uLP       396126048 - 141095037 = 255031011
// user1 unlocked uANC    0 + 92965637 = 92965637
// user1 unlocked uusd    1 + 214600473 = 214600474
// user1 unlocked uLP     141095037 - 141095037 = 0
//
// Step 3. cover
// UST needed amount = 341000000 - 214600474 = 126399526
// ANC sell amount = computeXykSwapInput(126399526, 168036530, 388280884) = 81466789
// ANC sell amount factored = 1.01 * 81466789 = 82281456
// UST actual return amount = computeXykSwapOutput(82281456, 168036530, 388280884) = 127248034
// return amount after tax = deductTax(127248034) = 127120913
// transfer cost = addTax(127120913) = 127248033
// ---
// primary pool uANC      168036530 + 82281456 = 250317986
// primary pool uusd      388280884 - 127248033 = 261032851
// user1 unlocked uANC    92965637 - 82281456 = 10684181
// user1 unlocked uusd    214600474 + 127120913 = 341721387
//
// Step 4. repay
// user1's debt amount: 341000000. repay this amount
// transaction cost: addTax(341000000) = 341341000
// user1's debt units is reduced to zero
// ---
// debt                   400164748 - 341000000 = 59164748
// total debt units       381109283809524 - 324761904761905 = 56347379047619
// user1 debt units       324761904761905 - 324761904761905 = 0
// user1 unlocked uusd    341721387 - 341341000 = 380387
//
// Step 5. refund the liquidator
// ANC bonus amount = 10684181 * 0.05 = 534209
// UST bonus amount = 380387 * 0.05 = 19019
// UST to send: deductTax(19019) = 19000
// transaction cost: addTax(19000) = 19019
// ---
// user1 unlocked uANC    11498848 - 534209 = 10964639
// user1 unlocked uusd    380387 - 19019 = 361368
//
// Step 6. refund the user
// refund all the remaining unlocked uusd to user
// UST refund amount = 10457138
// UST to send: deductTax(361368) = 361006
// transaction cost: addTax(361006) = 361367
// ---
// user1 unlocked uANC    10964639 - 10964639 = 0
// user1 unlocked uusd    361368 - 361367 = 1
//
// Result
// ---
// total bond units       84074777488481
// total debt units       56347379047619
// pending reward uASTRO  4000000
// pending reward uANC    2000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   84795880
// debt                   59164748
// primary pool uANC      250317986
// primary pool uusd      261032851
// primary pool uLP       255031011
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// State health:
// ANC price = computeXykSwapOutput(1000000, 250317986, 261032851) / 1e6 = 1.038655
// primary value = 250317986 * 1.038655 = 259994027
// secondary value = 261032851 * 1 = 261032851
// pool value = 2 * sqrt(259994027 * 261032851) = 521025842
// total bond value = 521025842 * 84795880 / 255031011 = 173237147
// total debt value = 59164748
//
// User1 health:
// bondValue = 0
// debtValue = 0
// ltv = null
//
// User2 health:
// bond and debt values are the same as the state's as user2 is the only user now
// ltv = 59164748 / 173237147 = 0.34152460384261581(0)
//--------------------------------------------------------------------------------------------------

async function testLiquidation() {
  process.stdout.write("\n9. Liquidation user 1... ");
  const { txhash } = await sendTransaction(terra, liquidator, [
    new MsgExecuteContract(liquidator.key.accAddress, field, {
      liquidate: {
        user: user1.key.accAddress,
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "84795880",
    debt: "59164748",
    ancUstPool: {
      assets: [
        { amount: "261032851" }, // uusd
        { amount: "250317986" }, // uANC
      ],
      total_share: "255031011",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "84074777488481",
      total_debt_units: "56347379047619",
      pending_rewards: [
        { amount: "1" },       // uusd
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
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84074777488481",
          debt_units: "56347379047619",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "173237147",
          debt_value: "59164748",
          ltv: "0.34152460384261581",
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
// total bond units       84074777488481
// total debt units       56347379047619
// pending reward uASTRO  4000000
// pending reward uANC    2000000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       84074777488481
// user2 debt units       56347379047619
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   84795880
// debt                   59164748
// primary pool uANC      250317986
// primary pool uusd      261032851
// primary pool uLP       255031011
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//
// Step 1. unbond
// unbond all of user2's 84795880 uLP
// contract receives 1000000 uASTRO + 500000 uANC staking reward
// ---
// bond                   0
// total bond units       0
// user2 bond units       0
// user2 unlocked uLP     0 + 84795880 = 84795880
// pending reward uASTRO  4000000 + 1000000 = 5000000
// pending reward uANC    2000000 + 500000 = 2500000
//
// Step 2. remove liquidity
// burn all of user2's 84795880 uLP
// ANC to be released: 250317986 * 84795880 / 255031011 = 83228834
// UST to be released: 261032851 * 84795880 / 255031011 = 86791446
// UST to receive: deductTax(86791446) = 86704741
// transaction cost for pool: addTax(86704741) = 86791445
// ---
// primary pool uANC      250317986 - 83228834 = 167089152
// primary pool uusd      261032851 - 86791445 = 174241406
// primary pool uLP       255031011 - 84795880 = 170235131
// user2 unlocked uANC    0 + 83228834 = 83228834
// user2 unlocked uusd    1 + 86791446 = 86791447
// user2 unlocked uLP     0
//
// Step 3. repay
// user2's remaining debts: 59164748 uusd
// transaction cost: addTax(59164748) = 59223912 uusd
// ---
// debt                   0
// total debt units       0
// user2 debt units       0
// user2 unlocked uusd    86791447 - 59223912 = 27567535
//
// Step 5. refund
// send all 27567535 uANC to user2
// UST to send: deductTax(27567535) = 27539995
// transaction cost: addTax(27539995) = 27567534
// ---
// user2 unlocked uANC    0
// user2 unlocked uusd    1
//
// Result
// ---
// total bond units       0
// total debt units       0
// pending reward uASTRO  5000000
// pending reward uANC    2500000
// pending reward uusd    1
// pending reward uLP     0
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uANC    0
// user1 unlocked uusd    1
// user1 unlocked uLP     0
// user2 bond units       0
// user2 debt units       0
// user2 unlocked uANC    0
// user2 unlocked uusd    1
// user2 unlocked uLP     0
// bond                   0
// debt                   0
// primary pool uANC      167089152
// primary pool uusd      174241406
// primary pool uLP       170235131
// astro pool uASTRO      101600000
// astro pool uusd        147644883
// astro pool uLP         122474487
//--------------------------------------------------------------------------------------------------

async function testReducePosition2() {
  process.stdout.write("\n10. User 2 closhing position... ");
  const { txhash } = await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, field, {
      update_position: [
        {
          unbond: {
            bond_units_to_reduce: "84074777488481",
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
    ancUstPool: {
      assets: [
        { amount: "174241406" }, // uusd
        { amount: "167089152" }, // uANC
      ],
      total_share: "170235131",
    },
    astroUstPool: {
      assets: [
        { amount: "147644883" }, // uusd
        { amount: "101600000" }, // uASTRO
      ],
      total_share: "122474487",
    },
    state: {
      total_bond_units: "0",
      total_debt_units: "0",
      pending_rewards: [
        { amount: "1" },       // uusd
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
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            { amount: "1" }, // uusd
          ],
        },
        health: {
          bond_value: "0",
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
