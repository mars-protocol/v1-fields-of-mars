import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction } from "../helpers/tx";
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

let astroToken: string;
let astroportFactory: string;
let lunaUstPair: string;
let lunaUstLpToken: string;
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
  const { cw20CodeId, address } = await deployCw20Token(
    deployer,
    undefined,
    "Astroport Token",
    "ASTRO"
  );
  astroToken = address;

  ({ astroportFactory } = await deployAstroportFactory(deployer, cw20CodeId));

  let { astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      native_token: {
        denom: "uluna",
      },
    },
    {
      native_token: {
        denom: "uusd",
      },
    },
  ]);
  lunaUstPair = astroportPair;
  lunaUstLpToken = astroportLpToken;

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      token: {
        contract_addr: astroToken,
      },
    },
    {
      native_token: {
        denom: "uusd",
      },
    },
  ]));
  astroUstPair = astroportPair;
  astroUstLpToken = astroportLpToken;

  ({ astroGenerator } = await deployAstroGenerator(deployer, lunaUstLpToken, astroToken));

  ({ oracle } = await deployOracle(deployer));

  ({ bank } = await deployRedBank(deployer));

  config = {
    primary_asset_info: {
      native: "uluna",
    },
    secondary_asset_info: {
      native: "uusd",
    },
    astro_token_info: {
      cw20: astroToken,
    },
    primary_pair: {
      contract_addr: lunaUstPair,
      liquidity_token: lunaUstLpToken,
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
    max_ltv: "0.83",
    fee_rate: "0",
    bonus_rate: "0.05",
  };
  ({ field } = await deployMartianField(deployer, config));

  process.stdout.write("Configuring LUNA and UST price oracle... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          native: {
            denom: "uluna",
          },
        },
        price_source: {
          fixed: {
            price: "50", // 1 uluna = 50 uusd
          },
        },
      },
    }),
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
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Astro generator contract with ASTRO... ");
  await sendTransaction(deployer, [
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

  // deployer provides 100 LUNA + 5000 UST
  // should receive sqrt(100000000 * 5000000000) = 707106781 uLP
  process.stdout.write("Provide initial liquidity to LUNA-UST pair... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(
      deployer.key.accAddress,
      lunaUstPair,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                native_token: {
                  denom: "uluna",
                },
              },
              amount: "100000000",
            },
            {
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "5000000000",
            },
          ],
        },
      },
      {
        uluna: "100000000",
        uusd: "5000000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // initialize the verifier object
  verifier = new Verifier(terra, field, config);
}

//--------------------------------------------------------------------------------------------------
// Test 1. Open Position, Part 1
//
// Prior to execution:
// ---
// bond                   0
// debt                   0
// primary pool uluna     100000000
// primary pool uusd      5000000000
// primary pool uLP       707106781
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// Step 1. deposit
// contract receives 1000000 uluna + 0 uusd
// ---
// user1 unlocked uluna   0 + 1000000 = 1000000
//
// Step 2. borrow
// contract borrows 50000000 uusd
// ---
// total debt units       0 + 50000000000000 = 50000000000000
// user1 debt units       0 + 50000000000000 = 50000000000000
// user1 unlocked uusd    0 + 50000000 = 50000000
// debt                   0 + 50000000 = 50000000
//
// Step 3. provide liquidity
// sends 1000000 uluna + 50000000 uusd to primary pair
// mint amount = mint(707106781 * 1000000 / 100000000, 707106781 * 50000000 / 5000000000) = 7071067
// ---
// user1 unlocked uluna   1000000 - 1000000 = 0
// user1 unlocked uusd    50000000 - 50000000 = 0
// user1 unlocked uLP     0 + 7071067 = 7071067
// primary pool uluna     100000000 + 1000000 = 101000000
// primary pool uusd      5000000000 + 50000000 = 5050000000
// primary pool uLP       707106781 + 7071067 = 714177848
//
// Step 4. bond
// send 7071067 uLP to Astro generator
// contract should receive 1000000 uASTRO
// ---
// total bond units       0 + 7071067000000 = 7071067000000
// user1 bond units       0 + 7071067000000 = 7071067000000
// user1 unlocked uLP     7071067 - 7071067 = 0
// bond                   0 + 7071067 = 7071067
// pending reward uASTRO  0 + 1000000 = 1000000
//
// Result
// ---
// total bond units       7071067000000
// total debt units       50000000000000
// pending reward uASTRO  1000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   7071067
// debt                   50000000
// primary pool uluna     101000000
// primary pool uusd      5050000000
// primary pool uLP       714177848
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// State health:
// uluna price = 50
// primary value = 101000000 * 50 = 5050000000
// secondary value = 5050000000 * 1 = 5050000000
// pool value = 2 * sqrt(5050000000 * 5050000000) = 10100000000
// total bond value = 10100000000 * 7071067 / 714177848 = 99999988
// total debt value = 50000000
//
// User1 health:
// same as state as user is the only user now
// ltv: 50000000 / 99999988 = 0.5000000600000072(00)
//--------------------------------------------------------------------------------------------------

async function testOpenPosition1() {
  process.stdout.write("\n1. Opening position for user 1... ");
  const { txhash } = await sendTransaction(user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                native: "uluna",
              },
              amount: "1000000",
            },
          },
          {
            borrow: {
              amount: "50000000",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005",
            },
          },
        ],
      },
      {
        uluna: "1000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "7071067",
    debt: "50000000",
    primaryPair: {
      assets: [
        { amount: "101000000" }, // uluna
        { amount: "5050000000" }, // uusd
      ],
      total_share: "714177848",
    },
    astroPair: {
      assets: [
        { amount: "0" }, // uASTRO
        { amount: "0" }, // uusd
      ],
      total_share: "0",
    },
    state: {
      total_bond_units: "7071067000000",
      total_debt_units: "50000000000000",
      pending_rewards: [
        { amount: "1000000" }, // uASTRO
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "50000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99999988",
          debt_value: "50000000",
          ltv: "0.5000000600000072",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 2. Open Position, Part 2
//
// Prior to execution
// ---
// total bond units       7071067000000
// total debt units       50000000000000
// pending reward uASTRO  1000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// bond                   7071067
// debt                   50000000
// primary pool uluna     101000000
// primary pool uusd      5050000000
// primary pool uLP       714177848
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// Step 1. deposit
// contract receives 1000000 uluna + 30000000 uusd
// ---
// user2 unlocked uluna   0 + 1000000 = 1000000
// user2 unlocked uusd    0 + 30000000 = 30000000
//
// Step 2. borrow
// contract borrows 20000000 uusd
// debt units to add: 50000000000000 * 20000000 / 50000000 = 20000000000000
// ---
// total debt units       50000000000000 + 20000000000000 = 70000000000000
// user2 debt units       0 + 20000000000000 = 20000000000000
// user2 unlocked uusd    30000000 + 20000000 = 50000000
// debt                   50000000 + 20000000 = 70000000
//
// Step 3. provide liquidity
// sends 1000000 uluna + 50000000 uusd to primary pair
// mint amount = mint(714177848 * 1000000 / 101000000, 714177848 * 50000000 / 5050000000) = 7071067
// ---
// user2 unlocked uluna   1000000 - 1000000 = 0
// user2 unlocked uusd    50000000 - 50000000 = 0
// user2 unlocked uLP     0 + 7071067 = 7071067
// primary pool uluna     101000000 + 1000000 = 102000000
// primary pool uusd      5050000000 + 50000000 = 5100000000
// primary pool uLP       714177848 + 7071067 = 721248915
//
// Step 4. bond
// send 7071067 uLP to Astro generator
// contract should receive 1000000 uASTRO
// bond units to add: 7071067000000 * 7071067 / 7071067 = 7071067000000
// ---
// total bond units       7071067000000 + 7071067000000 = 14142134000000
// user2 bond units       0 + 7071067000000 = 7071067000000
// user2 unlocked uLP     7071067 - 7071067 = 0
// bond                   7071067 + 7071067 = 14142134
// pending reward uASTRO  1000000 + 1000000 = 2000000
//
// Result
// ---
// total bond units       14142134000000
// total debt units       70000000000000
// pending reward uASTRO  2000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   14142134
// debt                   70000000
// primary pool uluna     102000000
// primary pool uusd      5100000000
// primary pool uLP       721248915
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// State health:
// uluna price = 50
// primary value = 102000000 * 50 = 5100000000
// secondary value = 5100000000 * 1 = 5100000000
// pool value = 2 * sqrt(5100000000 * 5100000000) = 10200000000
// total bond value = 10200000000 * 14142134 / 721248915 = 199999977
// total debt value = 70000000
//
// User1 health:
// user1 bond value: 199999977 * 7071067000000 / 14142134000000 = 99999988
// user1 debt value: 70000000 * 50000000000000 / 70000000000000 = 50000000
// ltv: 50000000 / 99999988 = 0.5000000600000072(00)
//
// User2 health:
// user2 bond value: 199999977 * 7071067000000 / 14142134000000 = 99999988
// user2 debt value: 70000000 * 20000000000000 / 70000000000000 = 20000000
// ltv: 20000000 / 99999988 = 0.20000002400000288(0)
//--------------------------------------------------------------------------------------------------

async function testOpenPosition2() {
  process.stdout.write("\n1. Opening position for user 2... ");
  const { txhash } = await sendTransaction(user2, [
    new MsgExecuteContract(
      user2.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                native: "uluna",
              },
              amount: "1000000",
            },
          },
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "30000000",
            },
          },
          {
            borrow: {
              amount: "20000000",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005",
            },
          },
        ],
      },
      {
        uluna: "1000000",
        uusd: "30000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "14142134",
    debt: "70000000",
    primaryPair: {
      assets: [
        { amount: "102000000" }, // uluna
        { amount: "5100000000" }, // uusd
      ],
      total_share: "721248915",
    },
    astroPair: {
      assets: [
        { amount: "0" }, // uASTRO
        { amount: "0" }, // uusd
      ],
      total_share: "0",
    },
    state: {
      total_bond_units: "14142134000000",
      total_debt_units: "70000000000000",
      pending_rewards: [
        { amount: "2000000" }, // uASTRO
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "50000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99999988",
          debt_value: "50000000",
          ltv: "0.5000000600000072",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "20000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99999988",
          debt_value: "20000000",
          ltv: "0.20000002400000288",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 3. Accrue Interest
//
// Prior to execution
// ---
// total bond units       14142134000000
// total debt units       70000000000000
// pending reward uASTRO  2000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   14142134
// debt                   70000000
// primary pool uluna     102000000
// primary pool uusd      5100000000
// primary pool uLP       721248915
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// We assume that the contract's debt amount suddenly more than doubles to 150 UST
//
// Result
// ---
// total bond units       14142134000000
// total debt units       70000000000000
// pending reward uASTRO  2000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   14142134
// debt                   150000000
// primary pool uluna     102000000
// primary pool uusd      5100000000
// primary pool uLP       721248915
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// State health:
// total bond value = 199999977 (unchanged)
// total debt value = 150000000
//
// User1 health:
// user1 bond value: 99999988 (unchanged)
// user1 debt value: 150000000 * 50000000000000 / 70000000000000 = 107142857
// ltv: 107142857 / 99999988 = 1.071428698571443828
//
// User2 health:
// user2 bond value: 99999988 (unchanged)
// user2 debt value: 150000000 * 20000000000000 / 70000000000000 = 42857142
// ltv: 42857142 / 99999988 = 0.428571471428576571
//--------------------------------------------------------------------------------------------------

async function testAccrueInterest() {
  process.stdout.write("\n2. Accruing interest... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bank, {
      set_user_debt: {
        user_address: field,
        denom: "uusd",
        amount: "150000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "14142134",
    debt: "150000000",
    primaryPair: {
      assets: [
        { amount: "102000000" }, // uluna
        { amount: "5100000000" }, // uusd
      ],
      total_share: "721248915",
    },
    astroPair: {
      assets: [
        { amount: "0" }, // uASTRO
        { amount: "0" }, // uusd
      ],
      total_share: "0",
    },
    state: {
      total_bond_units: "14142134000000",
      total_debt_units: "70000000000000",
      pending_rewards: [
        { amount: "2000000" }, // uASTRO
      ],
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "50000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99999988",
          debt_value: "107142857",
          ltv: "1.071428698571443828",
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "20000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99999988",
          debt_value: "42857142",
          ltv: "0.428571471428576571",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 4. Liquidate with Bad Debt
//
// Prior to execution
// ---
// total bond units       14142134000000
// total debt units       70000000000000
// pending reward uASTRO  2000000
// user1 bond units       7071067000000
// user1 debt units       50000000000000
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   14142134
// debt                   150000000
// primary pool uluna     102000000
// primary pool uusd      5100000000
// primary pool uLP       721248915
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// Step 1. unbond
// reduce all of user1's 7071067000000 bond units
// amount to unbond: 14142134 * 7071067000000 / 14142134000000 = 7071067
// upon unbonding, we automatically claim 1000000 uASTRO reward
// ---
// bond                   14142134 - 7071067 = 7071067
// total bond units       14142134000000 - 7071067000000 = 7071067000000
// user1 bond units       7071067000000 - 7071067000000 = 0
// user1 unlocked uLP     0 + 7071067 = 7071067
// pending reward uASTRO  2000000 + 1000000 = 3000000
//
// Step 2. remove liquidity
// burn all of user's 7071067 uLP
// uluna to be released: 102000000 * 7071067 / 721248915 = 999999
// uusd to be released: 5100000000 * 7071067 / 721248915 = 49999994
// ---
// primary pool uluna     102000000 - 999999 = 101000001
// primary pool uusd      5100000000 - 49999994 = 5050000006
// primary pool uLP       721248915 - 7071067 = 714177848
// user1 unlocked uluna   0 + 999999 = 999999
// user1 unlocked uusd    0 + 49999994 = 49999994
// user1 unlocked uLP     7071067 - 7071067 = 0
//
// Step 3. cover
// uusd needed amount: 107142857 - 49999994 = 57142863
// uluna sell amount = computeXykSwapInput(57142863, 101000001, 5050000006) = 1159455
// to account for numerical issue, we increment this amount by 1: 1159455 + 1 = 1159456
// this amount is greater than the available amount (999999), so we set sell amount to 999999
// uusd actual return amount = computeXykSwapOutput(999999, 101000001, 5050000006) = 49361225
// ---
// primary pool uluna     101000001 + 999999 = 102000000
// primary pool uusd      5050000006 - 49361225 = 5000638781
// user1 unlocked uluna   999999 - 999999 = 0
// user1 unlocked uusd    49999994 + 49361225 = 99361219
//
// Step 4. repay
// user's debt amount: 107142857, however user only has 99361219 uusd available. repay the available
// amount
// debt units to reduce: 50000000000000 * 99361219 / 107142857 = 46368568928491
// ---
// debt                   150000000 - 99361219 = 50638781
// total debt units       70000000000000 - 46368568928491 = 23631431071509
// user1 debt units       50000000000000 - 46368568928491 = 3631431071509
// user1 unlocked uusd    99361219 - 99361219 = 0
//
// Step 5. refund the liquidator
// nothing to refund, skip
//
// Step 6. refund the user
// nothing to refund, skip
//
// Step 7. clear bad debt
// waive the user's 3631431071509 debt units, emits a bad debt event
// the amount of bad debt is 107142857 - 99361219 = 7781638 uusd
// ---
// total debt units       23631431071509 - 3631431071509 = 20000000000000
// user debt units        3631431071509 - 3631431071509 = 0
//
// Result
// ---
// total bond units       7071067000000
// total debt units       20000000000000
// pending reward uASTRO  3000000
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   7071067
// debt                   50638781
// primary pool uluna     102000000
// primary pool uusd      5000638781
// primary pool uLP       714177848
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// State health:
// uluna price = 5000638781 / 102000000 = 49.025870
// primary value = 102000000 * 49.025870 = 5000638740
// secondary value = 5000638781 * 1 = 5000638781
// pool value = 2 * sqrt(5000638740 * 5000638781) = 10001277520
// total bond value = 10001277520 * 7071067 / 714177848 = 99022538
// total debt value = 50638781
//
// User1 health:
// user1 bond value: 0
// user1 debt value: 0
// ltv: null
//
// User2 health:
// user2 bond value: 99022538 (same as state)
// user2 debt value: 50638781 (same as state)
// ltv: 50638781 / 99022538 = 0.511386417908213986
//--------------------------------------------------------------------------------------------------

async function testLiquidateWithBadDebt() {
  process.stdout.write("\n4. Liquidating user 1... ");
  const result = await sendTransaction(liquidator, [
    new MsgExecuteContract(liquidator.key.accAddress, field, {
      liquidate: {
        user: user1.key.accAddress,
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", result.txhash);

  process.stdout.write("Updating LUNA price... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          native: {
            denom: "uluna",
          },
        },
        price_source: {
          fixed: {
            price: "49.025870", // 1 uluna = 49.025870 uusd
          },
        },
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "7071067",
    debt: "50638781",
    primaryPair: {
      assets: [
        { amount: "102000000" }, // uluna
        { amount: "5000638781" }, // uusd
      ],
      total_share: "714177848",
    },
    astroPair: {
      assets: [
        { amount: "0" }, // uASTRO
        { amount: "0" }, // uusd
      ],
      total_share: "0",
    },
    state: {
      total_bond_units: "7071067000000",
      total_debt_units: "20000000000000",
      pending_rewards: [
        { amount: "3000000" }, // uASTRO
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
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "20000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99022538",
          debt_value: "50638781",
          ltv: "0.511386417908213986",
        },
      },
    ],
  });
}

//--------------------------------------------------------------------------------------------------
// Test 5. Altruistic Debt Payment
//
// Prior to execution
// ---
// total bond units       7071067000000
// total debt units       20000000000000
// pending reward uASTRO  3000000
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   7071067
// debt                   50638781
// primary pool uluna     102000000
// primary pool uusd      5000638781
// primary pool uLP       714177848
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// a person repays user1's 7781638 uusd bad debt
// ---
// debt                   50638781 - 7781638 = 42857143
//
// Result
// ---
// total bond units       7071067000000
// total debt units       20000000000000
// pending reward uASTRO  3000000
// user1 bond units       0
// user1 debt units       0
// user1 unlocked uluna   0
// user1 unlocked uusd    0
// user1 unlocked uLP     0
// user2 bond units       7071067000000
// user2 debt units       20000000000000
// user2 unlocked uluna   0
// user2 unlocked uusd    0
// user2 unlocked uLP     0
// bond                   7071067
// debt                   42857143
// primary pool uluna     102000000
// primary pool uusd      5000638781
// primary pool uLP       714177848
// astro pool uASTRO      0
// astro pool uusd        0
// astro pool uLP         0
//
// State health:
// total bond value = 99022538 (unchanged)
// total debt value = 42857143
//
// User2 health:
// user2 bond value: 99022538 (unchanged)
// user2 debt value: 42857143 (same as state)
// ltv: 42857143 / 99022538 = 0.432801904148326313 (close to the value in test 3)
//--------------------------------------------------------------------------------------------------

async function testAltruisticDebtPayment() {
  process.stdout.write("\n5. Simulate an altruistic debt payment... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bank, {
      set_user_debt: {
        user_address: field,
        denom: "uusd",
        amount: "42857143",
      },
    }),
  ]);
  console.log(chalk.green("Done!"), "\ntxhash:", txhash);

  await verifier.verify({
    bond: "7071067",
    debt: "42857143",
    primaryPair: {
      assets: [
        { amount: "102000000" }, // uluna
        { amount: "5000638781" }, // uusd
      ],
      total_share: "714177848",
    },
    astroPair: {
      assets: [
        { amount: "0" }, // uASTRO
        { amount: "0" }, // uusd
      ],
      total_share: "0",
    },
    state: {
      total_bond_units: "7071067000000",
      total_debt_units: "20000000000000",
      pending_rewards: [
        { amount: "3000000" }, // uASTRO
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
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "7071067000000",
          debt_units: "20000000000000",
          unlocked_assets: [],
        },
        health: {
          bond_value: "99022538",
          debt_value: "42857143",
          ltv: "0.432801904148326313",
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

  console.log(`deployer   : ${chalk.cyan(deployer.key.accAddress)}`);
  console.log(`treasury   : ${chalk.cyan(treasury.key.accAddress)}`);
  console.log(`user 1     : ${chalk.cyan(user1.key.accAddress)}`);
  console.log(`user 2     : ${chalk.cyan(user1.key.accAddress)}`);
  console.log(`liquidator : ${chalk.cyan(liquidator.key.accAddress)}`);

  console.log(chalk.yellow("\nSetup"));

  await setupTest();

  console.log(chalk.yellow("\nTests"));

  await testOpenPosition1();
  await testOpenPosition2();
  await testAccrueInterest();
  await testLiquidateWithBadDebt();
  await testAltruisticDebtPayment();

  console.log(chalk.green("\nAll tests successfully completed!\n"));
})();
