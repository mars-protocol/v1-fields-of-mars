import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, toEncodedBinary } from "./helpers";
import {
  deployMartianField,
  deployMockAnchor,
  deployMockMars,
  deployTerraswapPair,
  deployTerraswapToken,
} from "./fixture";
import { Checker, Config } from "./check";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

// LocalTerra instance
const terra = new LocalTerra();

// User addresses
const deployer = terra.wallets.test1;
const treasury = terra.wallets.test2;
const user1 = terra.wallets.test3;
const user2 = terra.wallets.test4;
const liquidator1 = terra.wallets.test5;
const liquidator2 = terra.wallets.test6;

// Contract addresses
let anchorToken: string;
let anchorStaking: string;
let terraswapPair: string;
let terraswapLpToken: string;
let redBank: string;
let field: string;

// InstantiateMsg aka Config
let config: object;

// Helper for checking whether contract state matches expected values
let checker: Checker;

//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function setupTest() {
  // Part 1. Deploy mock contracts
  let { cw20CodeId, cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock Anchor Token",
    "ANC"
  );
  anchorToken = cw20Token;

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(terra, deployer, {
    asset_infos: [
      { native_token: { denom: "uusd" } },
      { token: { contract_addr: anchorToken } },
    ],
    token_code_id: cw20CodeId,
  }));

  anchorStaking = await deployMockAnchor(terra, deployer, anchorToken, terraswapLpToken);

  redBank = await deployMockMars(terra, deployer);

  // Part 2. Deploy Martian Field
  config = {
    long_asset: {
      token: {
        contract_addr: anchorToken,
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
      pair: terraswapPair,
      share_token: terraswapLpToken,
    },
    staking: {
      anchor: {
        contract_addr: anchorStaking,
        asset_token: anchorToken,
        staking_token: terraswapLpToken,
      },
    },
    keepers: [deployer.key.accAddress],
    treasury: treasury.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.75", // 75% debt ratio, i.e. 133.333...% collateralization ratio
    fee_rate: "0.2", // 20%
  };

  field = await deployMartianField(
    terra,
    deployer,
    "../artifacts/martian_field.wasm",
    config
  );

  // Part 3. Misc
  process.stdout.write("Fund deployer with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000", // 1000 ANC
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user1 with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user2 with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "34500000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Anchor Staking contract with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: anchorStaking,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Mars contract with UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(deployer.key.accAddress, redBank, { uusd: 99999000000 }), // 99999 UST
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Provide initial liquidity to TerraSwap Pair... ");

  // Deployer Provides 69 ANC + 420 UST
  // Should receive sqrt(69000000 * 420000000) = 170235131 uLP
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      terraswapPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "420000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
            {
              amount: "69000000",
              info: {
                token: {
                  contract_addr: anchorToken,
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

  // Finally, initialize the checker object
  checker = new Checker(terra, field, config as Config);
}

//----------------------------------------------------------------------------------------
// Test 1. Config
//----------------------------------------------------------------------------------------

async function testConfig() {
  await checker.check("null", "testConfig", {
    bond: {
      bond_amount: "0",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "0" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "420000000" },
        // uANC
        { amount: "69000000" },
      ],
      total_share: "170235131",
    },
    strategy: {
      state: {
        total_bond_units: "0",
        total_debt_units: "0",
      },
      health: {
        bond_value: "0",
        debt_value: "0",
        ltv: null,
      },
    },
    users: [],
  });
}

//----------------------------------------------------------------------------------------
// Test 2. Open Position, Pt. 1
//
// Prior to execution:
// ---
// bond      0 LP
// debt      0 uusd
// pool ANC  69000000 uANC
// pool UST  420000000 uusd
// pool LP   170235131 uLP
//
// Step 1. deposit
// contract receives 69000000 uANC & 0 uusd
// ---
// user1 unlocked uANC  0 + 69000000 = 69000000
//
// Step 2. borrow
// attempts to borrow 420000000 uusd; receives deductTax(420000000) = 419580419 uusd
// ---
// total debt unit      0 + 420000000000000 = 420000000000000
// user1 debt unit      0 + 420000000000000 = 420000000000000
// user1 unlocked uusd  0 + 419580419 = 419580419
// debt                 0 + 420000000 = 420000000
//
// Step 3. provide liquidity
// sends 69000000 uANC + deductTax(419580419) = 419161257 uusd to pool
// total tx cost is addTax(419161257) = 419580418 uusd
// expects to receive 419161257 * 170235131 / 420000000 = 169895170 uLP
// ---
// user1 unlocked uANC  69000000 - 69000000 = 0
// user1 unlocked uusd  419580419 - 419580418 = 1
// user1 unlocked uLP   0 + 169895170 = 169895170
// pool uANC            69000000 + 69000000 = 138000000
// pool uusd            420000000 + 419161257 = 839161257
// pool uLP             170235131 + 169895170 = 340130301
//
// Step 4. bond
// send 169895170 uLP to staking contract
// ---
// total bond units     0 + 169895170000000 = 169895170000000
// user1 bond units     0 + 169895170000000 = 169895170000000
// user1 unlocked uLP   169895170 - 169895170 = 0
// bond                 0 + 169895170 = 169895170
//
// Result
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 169895170
// debt                 420000000
// pool uANC            138000000
// pool uusd            839161257
// pool uLP             340130301
//
// ancPrice = computeXykSwapOutput(1000000, 138000000, 839161257) / 1000000
// = 6.037132 uusd
//
// State health:
// bondValue = (138000000 * 6.037132 + 839161257) * 169895170 / 340130301
// = 835307009 uusd
// debtValue = 420000000 uusd
// ltv = 420000000 / 835307009 = 0.502809141399171475
//
// User1 health:
// same as state as user1 is the only user now
//----------------------------------------------------------------------------------------

async function testOpenPosition1() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: field,
      },
    }),
    new MsgExecuteContract(user1.key.accAddress, field, {
      increase_position: {
        deposits: [
          {
            info: {
              token: {
                contract_addr: anchorToken,
              },
            },
            amount: "69000000",
          },
          {
            info: {
              native_token: {
                denom: "uusd",
              },
            },
            amount: "0",
          },
        ],
      },
    }),
  ]);

  await checker.check(txhash, "testOpenPosition1", {
    bond: {
      bond_amount: "169895170",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "420000000" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "839161257" },
        // uANC
        { amount: "138000000" },
      ],
      total_share: "340130301",
    },
    strategy: {
      state: {
        total_bond_units: "169895170000000",
        total_debt_units: "420000000000000",
      },
      health: {
        bond_value: "835307009",
        debt_value: "420000000",
        ltv: "0.502809141399171475",
      },
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "835307009",
          debt_value: "420000000",
          ltv: "0.502809141399171475",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 3. Harvest
//
// Prior to execution:
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 169895170
// debt                 420000000
// pool uANC            138000000
// pool uusd            839161257
// pool uLP             340130301
//
// Step 1. reinvest
// receives 1000000 uANC, sends 200000 uANC to treasury, swap 400000 uANC for UST
// 1.0 ANC reward claimed, 0.2 ANC charged as performance fee, 0.4 ANC swapped for UST
// kValue = poolUst * poolAnc = 839161257 * 138000000
// = 115804253466000000
// returnUst = poolUst - k / (poolAnc + sendAnc)
// = 839161257 - 115804253466000000 / (138000000 + 400000)
// = 2425321 uusd
// fee = returnUst * feeRate = 2425321 * 0.003
// = 7275 uusd
// returnUstAfterFee = returnUst - fee = 2425321 - 7275
// = 2418046 uusd
// returnUstAfterFeeAndTax = deductTax(returnUstAfterFee) = deductTax(2418046)
// = 2415630 uusd
// ustCostForPool = addTax(2415630) = 2418045 uusd
// ---
// field unlocked uANC  0 + 400000 = 400000
// field unlocked uusd  0 + 2415630 = 2415630
// pool uANC            138000000 + 400000 = 138400000
// pool uusd            839161257 - 2418045 = 836743212
//
// Step 2. provide liquidity
// sends: 400000 uANC + deductTax(2415631) = 2413216 uusd to pool
// total tx cost: addTax(2413216) = 2415629 uusd
// expects to receive: 2413216 * 340130301 / 836743212 = 980955 uLP
// ---
// Note: here my JS script incorrectly calculates deductTax(2415631) as 2413217; however,
// the Rust function in the contract calculates as 2413216
// ---
// field unlocked uANC  400000 - 400000 = 0
// field unlocked uusd  2415630 - 2415629 = 1
// field unlocked uLP   0 + 980955 = 980955
// pool uANC            138400000 + 400000 = 138800000
// pool uusd            836743212 + 2413216 = 839156428
// pool uLP             340130301 + 980955 = 341111256
//
// Step 4. bond
// send 341111256 uLP to staking contract
// ---
// Bond units should not change in a harvest transaction
// ---
// field unlocked uLP   980955 - 980955 = 0
// bond                 169895170 + 980955 = 170876125
//
// Result
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 170876125
// debt                 420000000
// pool uANC            138800000
// pool uusd            839156428
// pool uLP             341111256
//
// ancPrice = computeXykSwapOutput(1000000, 138800000, 839156428) / 1000000
// = 6.002550 uusd
//
// State health:
// bondValue = (138800000 * 6.002550 + 839156428) * 170876125 / 341111256
// = 837726432 uusd
// debtValue = 420000000 uusd
// ltv = 420000000 / 837726432 = 0.501356987145894424
//
// User1 health:
// same as state as user1 is the only user now
//----------------------------------------------------------------------------------------

async function testHarvest() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, field, {
      harvest: {},
    }),
  ]);

  await checker.check(txhash, "testHarvest", {
    bond: {
      bond_amount: "170876125",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "420000000" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "839156428" },
        // uANC
        { amount: "138800000" },
      ],
      total_share: "341111256",
    },
    strategy: {
      state: {
        total_bond_units: "169895170000000",
        total_debt_units: "420000000000000",
      },
      health: {
        bond_value: "837726432",
        debt_value: "420000000",
        ltv: "0.501356987145894424",
      },
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "837726432",
          debt_value: "420000000",
          ltv: "0.501356987145894424",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 4. Accrue Interest
//
// Prior to execution:
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 170876125
// debt                 420000000
// pool uANC            138800000
// pool uusd            839156428
// pool uLP             341111256
//
// We forcibly set the strategy's debt to 441000000 to simulate accrual of a 5% interest
//
// Result
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 170876125
// debt                 441000000
// pool uANC            138800000
// pool uusd            839156428
// pool uLP             341111256
//
// ancPrice = = 6.002550 uusd (unchanged)
//
// State health:
// bondValue = 837726432 uusd (unchanged)
// debtValue = 441000000 uusd
// ltv = 441000000 / 837726432 = 0.526424836503189146
//
// User1 health:
// same as state as user1 is the only user now
//----------------------------------------------------------------------------------------

async function testAccrueInterest() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, redBank, {
      set_debt: { user: field, denom: "uusd", amount: "441000000" },
    }),
  ]);

  await checker.check(txhash, "testAccrueInterest", {
    bond: {
      bond_amount: "170876125",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "441000000" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "839156428" },
        // uANC
        { amount: "138800000" },
      ],
      total_share: "341111256",
    },
    strategy: {
      state: {
        total_bond_units: "169895170000000",
        total_debt_units: "420000000000000",
      },
      health: {
        bond_value: "837726432",
        debt_value: "441000000",
        ltv: "0.526424836503189146",
      },
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "837726432",
          debt_value: "441000000",
          ltv: "0.526424836503189146",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 5. Open Position, Pt. 2
//
// Prior to execution:
// ---
// total bond units     169895170000000
// total debt units     420000000000000
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// bond                 170876125
// debt                 441000000
// pool uANC            138800000
// pool uusd            839156428
// pool uLP             341111256
//
// Step 1. deposit
// user deposits 34.5 ANC & 150 UST
// ---
// user2 unlocked uANC  0 + 34500000 = 34500000
// user2 unlocked uusd  0 + 150000000 = 150000000
//
// Step 2. borrow
// UST needed: 34500000 * 839156428 / 138800000 = 208579947 uusd
// UST to borrow: 208579947 - 150000000 = 58579947 uusd
// expects to receive: deductTax(58579947) = 58521425 uusd
// debt units to add: 58579947 * 420000000000000 / 441000000 = 55790425714285
// ---
// total debt units     420000000000000 + 55790425714285 = 475790425714285
// user2 debt units     0 + 55790425714285 = 55790425714285
// user2 unlocked uusd  150000000 + 58521425 = 208521425
// debt                 441000000 + 58579947 = 499579947
//
// Step 3. provide liquidity
// sends 34500000 uANC + deductTax(208521425) = 208313111 uusd to pool
// total tx cost is addTax(208313111) = 208521424 uusd
// expects to receive 208313111 * 341111256 / 839156428 = 84677831 uLP
// ---
// user1 unlocked uANC  34500000 - 34500000 = 0
// user1 unlocked uusd  208521425 - 208521424 = 1
// user1 unlocked uLP   0 + 84677831 = 84677831
// pool uANC            138800000 + 34500000 = 173300000
// pool uusd            839156428 + 208313111 = 1047469539
// pool uLP             341111256 + 84677831 = 425789087
//
// Step 4. bond
// send 84677831 uLP to staking contract
// bond units to add: 84677831 * 169895170000000 / 170876125 = 84191717789575
// ---
// total bond units     169895170000000 + 84191717789575 = 254086887789575
// user2 bond units     0 + 84191717789575 = 84191717789575
// user2 unlocked uLP   84677831 - 84677831 = 0
// bond                 170876125 + 84677831 = 255553956
//
// Result
// ---
// total bond units     254086887789575
// total debt units     475790425714285
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 499579947
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// ancPrice = computeXykSwapOutput(1000000, 173300000, 1047469539) / 1000000
// = 6.009579 uusd
//
// State health:
// bondValue = (173300000 * 6.009579 + 1047469539) * 255553956 / 425789087
// = 1253752700 uusd
// debtValue = 499579947
// ltv = 499579947 / 1253752700 = 0.398467693828296441
//
// User1 health:
// bondValue = 1253752700 * 169895170000000 / 254086887789575 = 838321607
// debtValue = 499579947 * 420000000000000 / 475790425714285 = 441000000
// ltv = 441000000 / 838321607 = 0.526051095805765149
//
// User2 health:
// bondValue = 1253752700 * 84191717789575 / 254086887789575 = 415431092
// debtValue = 499579947 * 55790425714285 / 475790425714285 = 58579946
// ltv = 58579946 / 415431092 = 0.141010018576077112
//----------------------------------------------------------------------------------------

async function testOpenPosition2() {
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
        increase_position: {
          deposits: [
            {
              info: {
                token: {
                  contract_addr: anchorToken,
                },
              },
              amount: "34500000",
            },
            {
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "150000000",
            },
          ],
        },
      },
      {
        uusd: "150000000",
      }
    ),
  ]);

  await checker.check(txhash, "testOpenPosition2", {
    bond: {
      bond_amount: "255553956",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "499579947" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "1047469539" },
        // uANC
        { amount: "173300000" },
      ],
      total_share: "425789087",
    },
    strategy: {
      state: {
        total_bond_units: "254086887789575",
        total_debt_units: "475790425714285",
      },
      health: {
        bond_value: "1253752700",
        debt_value: "499579947",
        ltv: "0.398467693828296441",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "838321607",
          debt_value: "441000000",
          ltv: "0.526051095805765149",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "415431092",
          debt_value: "58579946",
          ltv: "0.141010018576077112",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 6. Pay Debt
//
// Prior to execution:
// ---
// total bond units     254086887789575
// total debt units     475790425714285
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     420000000000000
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 499579947
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// Step 1. receiving user deposit
// user1 deposits 100 UST to contract
// ---
// user1 unlocked uusd  1 + 100000000 = 100000001
//
// Step 2. repay
// user1's outstanding debt value (441000000) is greater than unlocked uusd, so use all
// unlocked uusd to repay
// deliverable amount: deductTax(100000001) = 99900100
// transaction cost: addTax(99900100) = 100000000
// debt units to reduce: 475790425714285 * 99900100 / 499579947 = 95142952380952
// ---
// debt                 499579947 - 99900100 = 399679847
// total debt units     475790425714285 - 95142952380952 = 380647473333333
// user1 debt units     420000000000000 - 95142952380952 = 324857047619048
// user1 unlocked uusd  100000001 - 100000000 = 1
//
// Result
// ---
// total bond units     254086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 399679847
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// ancPrice = 6.009579 uusd (unchanged)
//
// State health:
// bondValue = 1253752700 uusd (unchanged)
// debtValue = 399679847
// ltv = 399679847 / 1253752700 = 0.318786828534845827
//
// User1 health:
// bondValue = 1253752700 * 169895170000000 / 254086887789575 = 838321607
// debtValue = 399679847 * 324857047619048 / 380647473333333 = 341099900
// ltv = 341099900 / 838321607 = 0.406884299714822929
//
// User2 health:
// bondValue = 1253752700 * 84191717789575 / 254086887789575 = 415431092
// debtValue = 399679847 * 55790425714285 / 380647473333333 = 58579946
// ltv = 58579946 / 415431092 = 0.141010018576077112
//----------------------------------------------------------------------------------------

async function testPayDebt() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      field,
      {
        pay_debt: {
          user: user1.key.accAddress,
          deposit: {
            info: {
              native_token: {
                denom: "uusd",
              },
            },
            amount: "100000000",
          },
        },
      },
      {
        uusd: "100000000",
      }
    ),
  ]);

  await checker.check(txhash, "testPayDebt", {
    bond: {
      bond_amount: "255553956",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "399679847" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "1047469539" },
        // uANC
        { amount: "173300000" },
      ],
      total_share: "425789087",
    },
    strategy: {
      state: {
        total_bond_units: "254086887789575",
        total_debt_units: "380647473333333",
      },
      health: {
        bond_value: "1253752700",
        debt_value: "399679847",
        ltv: "0.318786828534845827",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "324857047619048",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "838321607",
          debt_value: "341099900",
          ltv: "0.406884299714822929",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "415431092",
          debt_value: "58579946",
          ltv: "0.141010018576077112",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 7. Reduce Position, Pt. 1
//
// Prior to execution:
// ---
// total bond units     254086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     169895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 399679847
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// Step 1. unbond
// user1 has 169895170000000 bond units; we try reduce it by 30000000000000
// amount to unbond: 255553956 * 30000000000000 / 254086887789575 = 30173216
// ---
// bond                 255553956 - 30173216 = 225380740
// total bond units     254086887789575 - 30000000000000 = 224086887789575
// user1 bond units     169895170000000 - 30000000000000 = 139895170000000
// user1 unlocked uLP   0 + 30173216 = 30173216
//
// Step 2. remove liquidity
// skipped per message input
//
// Step 3. repay
// skipped per message input
//
// Step 4. refund
// send all 30173216 uLP to user1
// ---
// user1 unlocked uLP   30173216 - 30173216 = 0
//
// Result
// ---
// total bond units     224086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     139895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399679847
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// ancPrice = 6.009579 uusd (unchanged)
//
// State health:
// bondValue = (173300000 * 6.009579 + 1047469539) * 225380740 / 425789087
// = 1105722313 uusd
// debtValue = 399679847
// ltv = 399679847 / 1105722313 = 0.361464937716238344
//
// User1 health:
// bondValue = 1105722313 * 139895170000000 / 224086887789575 = 690291219
// debtValue = 399679847 * 324857047619048 / 380647473333333 = 341099900
// ltv = 341099900 / 690291219 = 0.49413912651842671(0)
//
// User2 health:
// bondValue = 1105722313 * 84191717789575 / 224086887789575 = 415431093
// debtValue = 399679847 * 55790425714285 / 380647473333333 = 58579946
// ltv = 58579946 / 415431093 =0.141010018236646528
//----------------------------------------------------------------------------------------

async function testReducePosition1() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, field, {
      reduce_position: {
        bond_units: "30000000000000",
        remove: false,
        repay: false,
      },
    }),
  ]);

  await checker.check(txhash, "testReducePosition1", {
    bond: {
      bond_amount: "225380740",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "399679847" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "1047469539" },
        // uANC
        { amount: "173300000" },
      ],
      total_share: "425789087",
    },
    strategy: {
      state: {
        total_bond_units: "224086887789575",
        total_debt_units: "380647473333333",
      },
      health: {
        bond_value: "1105722313",
        debt_value: "399679847",
        ltv: "0.361464937716238344",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324857047619048",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "690291219",
          debt_value: "341099900",
          ltv: "0.49413912651842671",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "415431093",
          debt_value: "58579946",
          ltv: "0.141010018236646528",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 8. Dump
//
// Prior to execution:
// ---
// total bond units     224086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     139895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399679847
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// We dump 35 ANC token in the AMM, which should barely make user1 liquidatable
// kValue = poolUst * poolAnc = 1047469539 * 173300000
// = 181526471108700000
// returnUst = poolUst - k / (poolAnc + sendAnc)
// = 1047469539 - 181526471108700000 / (173300000 + 100000000)
// = 383267303 uusd
// fee = returnUst * feeRate = 383267303 * 0.003
// = 1149801 uusd
// returnUstAfterFee = returnUst - fee = 383267303 - 1149801
// = 382117502 uusd
// returnUstAfterFeeAndTax = deductTax(returnUstAfterFee) = deductTax(382117502)
// = 381735766 uusd
// ustCostForPool = addTax(381735766) = 382117501 uusd
// ---
// pool uANC            173300000 + 100000000 = 273300000
// pool uusd            1047469539 - 382117501 = 665352038
//
// Result
// ---
// total bond units     224086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     139895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399679847
// pool uANC            273300000
// pool uusd            665352038
// pool uLP             425789087
//
// ancPrice = computeXykSwapOutput(1000000, 273300000, 665352038) / 1000000
// = 2.425637 uusd
//
// State health:
// bondValue = (273300000 * 2.425637 + 665352038) * 225380740 / 425789087
// = 703090872 uusd
// debtValue = 399679847
// ltv = 399679847 / 703090872 = 0.568461151917785102
//
// User1 health:
// bondValue = 703090872 * 139895170000000 / 224086887789575 = 438932496
// debtValue = 399679847 * 324857047619048 / 380647473333333 = 341099900
// ltv = 341099900 / 438932496 = 0.777112433252150918
//
// User2 health:
// bondValue = 703090872 * 84191717789575 / 224086887789575 = 264158375
// debtValue = 399679847 * 55790425714285 / 380647473333333 = 58579946
// ltv = 58579946 / 264158375 = 0.221760699428893746
//----------------------------------------------------------------------------------------

async function testDump() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      send: {
        amount: "100000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  await checker.check(txhash, "testDump", {
    bond: {
      bond_amount: "225380740",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "399679847" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "665352038" },
        // uANC
        { amount: "273300000" },
      ],
      total_share: "425789087",
    },
    strategy: {
      state: {
        total_bond_units: "224086887789575",
        total_debt_units: "380647473333333",
      },
      health: {
        bond_value: "703090872",
        debt_value: "399679847",
        ltv: "0.568461151917785102",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324857047619048",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "438932496",
          debt_value: "341099900",
          ltv: "0.777112433252150918",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "264158375",
          debt_value: "58579946",
          ltv: "0.221760699428893746",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 9. Liquidation, Pt. 1
//
//----------------------------------------------------------------------------------------
// Part 1. Close Position
//
// Prior to execution:
// ---
// total bond units     224086887789575
// total debt units     380647473333333
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     139895170000000
// user1 debt units     324857047619048
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399679847
// pool uANC            273300000
// pool uusd            665352038
// pool uLP             425789087
//
// Step 1. unbond
// reduce all of user1's bond units
// amount to unbond: 225380740 * 139895170000000 / 224086887789575 = 140702908
// ---
// bond                 225380740 - 140702908 = 84677832
// total bond units     224086887789575 - 139895170000000 = 84191717789575
// user1 bond units     139895170000000 - 139895170000000 = 0
// user1 unlocked uLP   0 + 140702908 = 140702908
//
// Step 2. remove liquidity
// burn of of user1's 140702908 uLP
// ANC to be released: 273300000 * 140702908 / 425789087 = 90312565
// UST to be released: 665352038 * 140702908 / 425789087 = 219866993
// UST to receive: deductTax(219866993) = 219647345
// transaction cost for pool: addTax(219647345) = 219866992
// ---
// pool uANC            273300000 - 90312565 = 182987435
// pool uusd            665352038 - 219866992 = 445485046
// pool uLP             425789087 - 140702908 = 285086179
// user1 unlocked uANC  0 + 90312565 = 90312565
// user1 unlocked uusd  1 + 219647345 = 219647346
// user1 unlocked uLP   140702908 - 140702908 = 0
//
// Step 3. repay
// user1's outstanding debt (341099900) is greater than his unlocked uusd (219647346)
// therefore, use all of the unlocked uusd to repay
// deliverable amount: deductTax(219647346) = 219427918
// transaction cost: addTax(219427918) = 219647345
// debt units to reduce: 380647473333333 * 219427918 / 399679847 = 208978969523809
// ---
// debt                 399679847 - 219427918 = 180251929
// total debt units     380647473333333 - 208978969523809 = 171668503809524
// user1 debt units     324857047619048 - 208978969523809 = 115878078095239
// user1 unlocked uusd  219647346 - 219647345 = 1
//
// Result
// ---
// total bond units     84191717789575
// total debt units     171668503809524
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     115878078095239
// user1 unlocked uANC  90312565
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 84677832
// debt                 180251929
// pool uANC            182987435
// pool uusd            445485046
// pool uLP             285086179
//----------------------------------------------------------------------------------------
// Part 2. Liquidate
//
// Step 1. deposit
// user1 remaining debt: 180251929 * 115878078095239 / 171668503809524 = 121671982
// liquidator provides 100000000 uusd
// deliverable amount: deductTax(100000000) = 99900099
// percentage: 99900099 / 121671982 = 0.821060833873816570
// ---
// user1 unlocked uusd  1 + 100000000 = 100000001
//
// Step 2. repay
// user1's outstanding debt (121671981) is greater than his unlocked uusd (100000001)
// therefore, use all of the unlocked uusd to repay
// deliverable amount: deductTax(100000001) = 99900100
// transaction cost: addTax(99900100) = 100000000
// debt units to reduce: 115878078095239 * 99900100 / 121671982 = 95142952380953
// ---
// debt                 180251929 - 99900100 = 80351829
// total debt units     171668503809524 - 95142952380953 = 76525551428571
// user1 debt units     115878078095239 - 95142952380953 = 20735125714286
// user1 unlocked uusd  100000001 - 100000000 = 1
//
// Step 3. refund
// ANC to refund: 90312565 * 0.821060833873816570 = 74152109
// UST to refund: 1 * 0.821060833873816570 = 0
// ---
// user1 unlocked uANC  90312565 - 74152109 = 16160456
//
// Result
// ---
// total bond units     84191717789575
// total debt units     76525551428571
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     20735125714286
// user1 unlocked uANC  16160456
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 84677832
// debt                 80351829
// pool uANC            182987435
// pool uusd            445485046
// pool uLP             285086179
//
// ancPrice = computeXykSwapOutput(1000000, 182987435, 445485046) / 1000000
// = 2.421280 uusd
//
// State health:
// bondValue = (182987435 * 2.421280 + 445485046) * 84677832 / 285086179
// = 263921567 uusd
// debtValue = 80351829
// ltv = 80351829 / 263921567 = 0.304453440138903085
//
// User1 health:
// bondValue = 0
// debtValue = 80351829 * 20735125714286 / 76525551428571 = 21771882
// ltv = null
//
// User2 health:
// bondValue = 263921567 (same as state)
// debtValue = 80351829 * 55790425714285 / 76525551428571 = 58579946
// ltv = 58579946 / 263921567 = 0.221959677891727582
//----------------------------------------------------------------------------------------

async function testLiquidation1() {
  const { txhash } = await sendTransaction(terra, liquidator1, [
    new MsgExecuteContract(liquidator1.key.accAddress, field, {
      close_position: {
        user: user1.key.accAddress,
      },
    }),
    new MsgExecuteContract(
      liquidator1.key.accAddress,
      field,
      {
        liquidate: {
          user: user1.key.accAddress,
          deposit: {
            info: {
              native_token: {
                denom: "uusd",
              },
            },
            amount: "100000000",
          },
        },
      },
      {
        uusd: "100000000",
      }
    ),
  ]);

  await checker.check(txhash, "testLiquidation1", {
    bond: {
      bond_amount: "84677832",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "80351829" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "445485046" },
        // uANC
        { amount: "182987435" },
      ],
      total_share: "285086179",
    },
    strategy: {
      state: {
        total_bond_units: "84191717789575",
        total_debt_units: "76525551428571",
      },
      health: {
        bond_value: "263921567",
        debt_value: "80351829",
        ltv: "0.304453440138903085",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "20735125714286",
          unlocked_assets: [
            // uANC
            { amount: "16160456" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "21771882",
          ltv: null,
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "263921567",
          debt_value: "58579946",
          ltv: "0.221959677891727582",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 10. Liquidation, Pt. 2
//
// Prior to execution:
// ---
// total bond units     84191717789575
// total debt units     76525551428571
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     20735125714286
// user1 unlocked uANC  16160456
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 84677832
// debt                 80351829
// pool uANC            182987435
// pool uusd            445485046
// pool uLP             285086179
//
// Step 1. deposit
// user1 has 21771882 uusd remaining debt
// liquidator provides 200000000 uusd (more than enough)
// percentage: 1
// ---
// user1 unlocked uusd  1 + 200000000 = 200000001
//
// Step 2. repay
// repay all the remaining debt: 21771882 uusd
// transaction cost: addTax(21771882) = 21793653
// reduce debt units to zero
// ---
// debt                 80351829 - 21771882 = 58579947
// total debt units     76525551428571 - 20735125714286 = 55790425714285
// user1 debt units     0
// user1 unlocked uusd  200000001 - 21793653 = 178206348
//
// Step 3. refund
// refund all ANC
// UST to refund: deductTax(178206348) = 178028319
// transaction cost: addTax(178028319) = 178206347
// ---
// user1 unlocked uANC  0
// user1 unlocked uusd  178206348 - 178206347 = 1
//
// Result
// ---
// total bond units     84191717789575
// total debt units     55790425714285
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     0
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 84677832
// debt                 58579947
// pool uANC            182987435
// pool uusd            445485046
// pool uLP             285086179
//
// ancPrice = 2.421280 uusd (unchanged)
//
// State health:
// bondValue = 263921567 uusd (unchanged)
// debtValue = 58579947
// ltv = 58579947 / 263921567 = 0.221959681680732063
//
// User1 health:
// bondValue = 0
// debtValue = 0
// ltv = null
//
// User2 health:
// same as state
//----------------------------------------------------------------------------------------

async function testLiquidation2() {
  const { txhash } = await sendTransaction(terra, liquidator2, [
    new MsgExecuteContract(
      liquidator2.key.accAddress,
      field,
      {
        liquidate: {
          user: user1.key.accAddress,
          deposit: {
            info: {
              native_token: {
                denom: "uusd",
              },
            },
            amount: "200000000",
          },
        },
      },
      {
        uusd: "200000000",
      }
    ),
  ]);

  await checker.check(txhash, "testLiquidation2", {
    bond: {
      bond_amount: "84677832",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "58579947" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "445485046" },
        // uANC
        { amount: "182987435" },
      ],
      total_share: "285086179",
    },
    strategy: {
      state: {
        total_bond_units: "84191717789575",
        total_debt_units: "55790425714285",
      },
      health: {
        bond_value: "263921567",
        debt_value: "58579947",
        ltv: "0.221959681680732063",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "263921567",
          debt_value: "58579947",
          ltv: "0.221959681680732063",
        },
      },
    ],
  });
}

//----------------------------------------------------------------------------------------
// Test 11. Reduce Position, Pt. 2
//
// Prior to execution:
// ---
// total bond units     84191717789575
// total debt units     55790425714285
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     0
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 84677832
// debt                 58579947
// pool uANC            182987435
// pool uusd            445485046
// pool uLP             285086179
//
// Step 1. unbond
// amount to unbond: 84677832 (all)
// ---
// bond                 0
// total bond units     0
// user1 bond units     0
// user1 unlocked uLP   0 + 84677832 = 84677832
//
// Step 2. remove liquidity
// burn of of user2's 84677832 uLP
// ANC to be released: 182987435 * 84677832 / 285086179 = 54351913
// UST to be released: 445485046 * 84677832 / 285086179 = 132320367
// UST to receive: deductTax(132320367) = 132188178
// transaction cost for pool: addTax(132188178) = 132320366
// ---
// pool uANC            182987435 - 54351913 = 128635522
// pool uusd            445485046 - 132320366 = 313164680
// pool uLP             285086179 - 84677832 = 200408347
// user2 unlocked uANC  0 + 54351913 = 54351913
// user2 unlocked uusd  1 + 132320367 = 132320368
// user2 unlocked uLP   0
//
// Step 3. repay
// repay all remaining debts: 58579947 uusd
// transaction cost: addTax(58579947) = 58638526 uusd
// ---
// debt                 0
// total debt units     0
// user2 debt units     0
// user2 unlocked uusd  132320368 - 58638526 = 73681842
//
// Step 4. refund
// send all 54351913 uANC to user2
// UST to send: deductTax(73681842) = 73608233
// transaction cost: addTax(73608233) = 73681841
// ---
// user2 unlocked uANC  0
// user2 unlocked uusd  1
//
// Result
// ---
// total bond units     0
// total debt units     0
// field unlocked uANC  0
// field unlocked uusd  1
// field unlocked uLP   0
// user1 bond units     0
// user1 debt units     0
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     0
// user2 debt units     0
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 0
// debt                 0
// pool uANC            128635522
// pool uusd            313164680
// pool uLP             200408347
//----------------------------------------------------------------------------------------

async function testReducePosition2() {
  const { txhash } = await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, field, {
      reduce_position: {
        bond_units: undefined, // gives `signature verification failed` error if use `null`
        remove: true,
        repay: true,
      },
    }),
  ]);

  await checker.check(txhash, "testReducePosition2", {
    bond: {
      bond_amount: "0",
    },
    debt: {
      debts: [
        // uluna
        { amount: "0" },
        // uusd
        { amount: "0" },
      ],
    },
    pool: {
      assets: [
        // uusd
        { amount: "313164680" },
        // uANC
        { amount: "128635522" },
      ],
      total_share: "200408347",
    },
    strategy: {
      state: {
        total_bond_units: "0",
        total_debt_units: "0",
      },
      health: {
        bond_value: "0",
        debt_value: "0",
        ltv: null,
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "0",
          ltv: null,
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            // uANC
            { amount: "0" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
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

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(treasury.key.accAddress)} as treasury`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);
  console.log(`Use ${chalk.cyan(liquidator1.key.accAddress)} as liquidator 1`);
  console.log(`Use ${chalk.cyan(liquidator2.key.accAddress)} as liquidator 2`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Martian Field: ANC-UST LP"));

  await testConfig();
  await testOpenPosition1();
  await testHarvest();
  await testAccrueInterest();
  await testOpenPosition2();
  await testPayDebt();
  await testReducePosition1();
  await testDump();
  await testLiquidation1();
  await testLiquidation2();
  await testReducePosition2();

  console.log(chalk.green("\nAll tests successfully completed. Hooray!\n"));
})();
