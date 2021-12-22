import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, toEncodedBinary } from "./helpers";
import {
  deployCw20Token,
  deployAstroport,
  deployRedBank,
  deployOracle,
  deployAnchorStaking,
  deployMartianField,
} from "./fixture";
import { Contract, Protocols, MartianField } from "./types";
import { Verifier } from "./verifier";

// LocalTerra instance
const terra = new LocalTerra();

// User addresses
const deployer = terra.wallets.test1;
const treasury = terra.wallets.test2;
const user1 = terra.wallets.test3;
const user2 = terra.wallets.test4;
const liquidator = terra.wallets.test5;

// Contract addresses
let astroport: Protocols.Astroport;
let anchor: Protocols.Anchor;
let mars: Protocols.Mars;
let field: Contract;

// InstantiateMsg aka Config
let config: MartianField.Config;

// Helper for checking whether contract state matches expected values
let verifier: Verifier;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  // Deploy mock Anchor token, staking, Astroport pair
  const token = await deployCw20Token(terra, deployer);
  astroport = await deployAstroport(terra, deployer, token);
  const staking = await deployAnchorStaking(terra, deployer, token, astroport);
  anchor = { token, staking };

  // Deploy mock Red Bank + Oracle
  const redBank = await deployRedBank(terra, deployer);
  const oracle = await deployOracle(terra, deployer);
  mars = { redBank, oracle };

  // Deploy Martian Field
  config = {
    primary_asset_info: {
      cw20: anchor.token.address,
    },
    secondary_asset_info: {
      native: "uusd",
    },
    red_bank: {
      contract_addr: mars.redBank.address,
    },
    oracle: {
      contract_addr: mars.oracle.address,
    },
    pair: {
      contract_addr: astroport.pair.address,
      liquidity_token: astroport.liquidityToken.address,
    },
    staking: {
      anchor: {
        contract_addr: anchor.staking.address,
        asset_token: anchor.token.address,
        staking_token: astroport.liquidityToken.address,
      },
    },
    treasury: treasury.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.75", // 75%, i.e. for every 100 UST asset there must be no more than 75 UST debt
    fee_rate: "0.2", // 20%
    bonus_rate: "0.05", // 5%
  };

  field = await deployMartianField(terra, deployer, config);

  // Other setups
  process.stdout.write("Configuring asset with oracle...");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle.address, {
      set_asset: {
        asset: {
          cw20: {
            contract_addr: anchor.token.address,
          },
        },
        price_source: {
          astroport_spot: {
            pair_address: astroport.pair.address,
            asset_address: anchor.token.address,
          },
        },
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund deployer with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000", // 1000 ANC
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user1 with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user2 with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "34500000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Anchor Staking contract with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: anchor.staking.address,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Mars contract with UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(deployer.key.accAddress, mars.redBank.address, { uusd: 99999000000 }), // 99999 UST
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Provide initial liquidity to Astroport Pair... ");

  // Deployer Provides 69 ANC + 420 UST
  // Should receive sqrt(69000000 * 420000000) = 170235131 uLP
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      increase_allowance: {
        amount: "69000000",
        spender: astroport.pair.address,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      astroport.pair.address,
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
                  contract_addr: anchor.token.address,
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

  // Finally, initialize the verifier object
  verifier = new Verifier(terra, field.address, config);
}

//--------------------------------------------------------------------------------------------------
// Test 1. Config
//--------------------------------------------------------------------------------------------------

async function testConfig() {
  await verifier.verify("null", "testConfig", {
    bond: {
      bond_amount: "0",
    },
    debt: {
      amount: "0",
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
    state: {
      total_bond_units: "0",
      total_debt_units: "0",
    },
    users: [],
  });
}

//--------------------------------------------------------------------------------------------------
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
// ancPrice = computeSwapOutput(1000000, 138000000, 839161257) / 1000000
// = 6.037132 uusd
//
// State health:
// totalBondValue = (138000000 * 6.037132 + 839161257) * 169895170 / 340130301
// = 835307009 uusd
// totalDebtValue = 420000000 uusd
//
// User1 health:
// same as state as user1 is the only user now
//--------------------------------------------------------------------------------------------------

async function testOpenPosition1() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, anchor.token.address, {
      increase_allowance: {
        amount: "69000000",
        spender: field.address,
      },
    }),
    new MsgExecuteContract(user1.key.accAddress, field.address, {
      update_position: [
        {
          deposit: {
            info: {
              cw20: anchor.token.address,
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
          bond: {},
        },
      ],
    }),
  ]);
  console.log("txhash:", txhash);

  await verifier.verify(txhash, "testOpenPosition1", {
    bond: {
      bond_amount: "169895170",
    },
    debt: {
      amount: "420000000",
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
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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

//--------------------------------------------------------------------------------------------------
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
// Step 1. swap
// receives 1000000 uANC, sends 200000 uANC to treasury, swap 400000 uANC for UST
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
// ancPrice = computeSwapOutput(1000000, 138800000, 839156428) / 1000000
// = 6.002550 uusd
//
// State health:
// totalBondValue = (138800000 * 6.002550 + 839156428) * 170876125 / 341111256
// = 837726432 uusd
// totalDebtValue = 420000000 uusd
//
// User1 health:
// same as state as user1 is the only user now
//--------------------------------------------------------------------------------------------------

async function testHarvest() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, field.address, {
      harvest: {},
    }),
  ]);

  await verifier.verify(txhash, "testHarvest", {
    bond: {
      bond_amount: "170876125",
    },
    debt: {
      amount: "420000000",
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
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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

//--------------------------------------------------------------------------------------------------
// Test 4. Accrue Interest
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
// totalBondValue = 837726432 uusd (unchanged)
// totalDebtValue = 441000000 uusd
//
// User1 health:
// same as state as user1 is the only user now
//--------------------------------------------------------------------------------------------------

async function testAccrueInterest() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mars.redBank.address, {
      set_user_debt: {
        user_address: field.address,
        denom: "uusd",
        amount: "441000000",
      },
    }),
  ]);

  await verifier.verify(txhash, "testAccrueInterest", {
    bond: {
      bond_amount: "170876125",
    },
    debt: {
      amount: "441000000",
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
    state: {
      total_bond_units: "169895170000000",
      total_debt_units: "420000000000000",
    },
    users: [
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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

//--------------------------------------------------------------------------------------------------
// Test 5. Open Position, Pt. 2
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
// ancPrice = computeSwapOutput(1000000, 173300000, 1047469539) / 1000000
// = 6.009579 uusd
//
// State health:
// totalBondValue = (173300000 * 6.009579 + 1047469539) * 255553956 / 425789087
// = 1253752700 uusd
// totalDebtValue = 499579947
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
//--------------------------------------------------------------------------------------------------

async function testOpenPosition2() {
  const { txhash } = await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, anchor.token.address, {
      increase_allowance: {
        amount: "34500000",
        spender: field.address,
      },
    }),
    new MsgExecuteContract(
      user2.key.accAddress,
      field.address,
      {
        update_position: [
          {
            deposit: {
              info: {
                cw20: anchor.token.address,
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
              amount: "58579947",
            },
          },
          {
            bond: {},
          },
        ],
      },
      {
        uusd: "150000000",
      }
    ),
  ]);

  await verifier.verify(txhash, "testOpenPosition2", {
    bond: {
      bond_amount: "255553956",
    },
    debt: {
      amount: "499579947",
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
    state: {
      total_bond_units: "254086887789575",
      total_debt_units: "475790425714285",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "420000000000000",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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

//--------------------------------------------------------------------------------------------------
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
// user1 deposits 100.1 UST to contract
// ---
// user1 unlocked uusd  1 + 100100000 = 100100001
//
// Step 2. repay
// Repay 100 UST
// transaction cost: addTax(100000000) = 100100000
// debt units to reduce: 475790425714285 * 100000000 / 499579947 = 95238095238095
// ---
// debt                 499579947 - 100000000 = 399579947
// total debt units     475790425714285 - 95238095238095 = 380552330476190
// user1 debt units     420000000000000 - 95238095238095 = 324761904761905
// user1 unlocked uusd  100100001 - 100100000 = 1
//
// Result
// ---
// total bond units     254086887789575
// total debt units     380552330476190
// user1 bond units     169895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 399579947
// pool uANC            173300000
// pool uusd            1047469539
// pool uLP             425789087
//
// ancPrice = 6.009579 uusd (unchanged)
//
// State health:
// totaoBondValue = 1253752700 uusd (unchanged)
// totalDebtValue = 399579947
//
// User1 health:
// bondValue = 1253752700 * 169895170000000 / 254086887789575 = 838321607
// debtValue = 399579947 * 324761904761905 / 380552330476190 = 341000000
// ltv = 341000000 / 838321607 = 0.40676513303801795(0)
//
// User2 health:
// bondValue = 1253752700 * 84191717789575 / 254086887789575 = 415431092
// debtValue = 399579947 * 55790425714285 / 380552330476190 = 58579946
// ltv = 58579946 / 415431092 = 0.141010018576077112
//--------------------------------------------------------------------------------------------------

async function testPayDebt() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      field.address,
      {
        update_position: [
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "100500000",
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
        uusd: "100500000", // 100.1 is actually needed; should refund us ~0.4 UST
      }
    ),
  ]);

  await verifier.verify(txhash, "testPayDebt", {
    bond: {
      bond_amount: "255553956",
    },
    debt: {
      amount: "399579947",
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
    state: {
      total_bond_units: "254086887789575",
      total_debt_units: "380552330476190",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "169895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "838321607",
          debt_value: "341000000",
          ltv: "0.40676513303801795",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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

//--------------------------------------------------------------------------------------------------
// Test 7. Reduce Position, Pt. 1
//
// Prior to execution:
// ---
// total bond units     254086887789575
// total debt units     380552330476190
// user1 bond units     169895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 255553956
// debt                 399579947
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
// burn of of user1's 30173216 uLP
// ANC to be released: 173300000 * 30173216 / 425789087 = 12280771
// UST to be released: 1047469539 * 30173216 / 425789087 = 74228122
// UST to receive: deductTax(74228122) = 74153968
// transaction cost for pool: addTax(74153968) = 74228121
// ---
// pool uANC            173300000 - 12280771 = 161019229
// pool uusd            1047469539 - 74228121 = 973241418
// pool uLP             425789087 - 30173216 = 395615871
// user1 unlocked uANC  0 + 161019229 = 161019229
// user1 unlocked uusd  1 + 973241418 = 973241419
// user1 unlocked uLP   30173216 - 30173216 = 0
//
// Step 3. swap
// skipped as `swap_amount` is zero
//
// Step 4. repay
// skipped as `repay_amount` is zero
//
// Step 5. refund
// send all 161019229 uANC to user1
// UST to send: deductTax(973241419) = 972269149
// transaction cost: addTax(972269149) = 973241418
// ---
// user1 unlocked uANC  161019229 - 161019229 = 0
// user1 unlocked uusd  973241419 - 973241418 = 1
//
// Result
// ---
// total bond units     224086887789575
// total debt units     380552330476190
// user1 bond units     139895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399579947
// pool uANC            161019229
// pool uusd            973241418
// pool uLP             395615871
//
// ancPrice = computeSwapOutput(1000000, 161019229, 973241418) / 1000000
// = 6.006951 uusd
//
// State health:
// totalBondValue = (161019229 * 6.006951 + 973241418) * 225380740 / 395615871
// = 1105481243 uusd
// totalDebtValue = 399579947 uusd
//
// User1 health:
// bondValue = 1105481243 * 139895170000000 / 224086887789575 = 690140721
// debtValue = 399579947 * 324761904761905 / 380552330476190 = 341000000
// ltv = 341000000 / 690140721 = 0.49410212964378898(0)
//
// User2 health:
// bondValue = 1105481243 * 84191717789575 / 224086887789575 = 415340521
// debtValue = 399579947 * 55790425714285 / 380552330476190 = 58579946
// ltv = 58579946 / 415340521 = 0.141040767847450164
//--------------------------------------------------------------------------------------------------

async function testReducePosition1() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, field.address, {
      update_position: [
        {
          unbond: {
            bond_units_to_reduce: "30000000000000",
          },
        },
      ],
    }),
  ]);

  await verifier.verify(txhash, "testReducePosition1", {
    bond: {
      bond_amount: "225380740",
    },
    debt: {
      amount: "399579947",
    },
    pool: {
      assets: [
        // uusd
        { amount: "973241418" },
        // uANC
        { amount: "161019229" },
      ],
      total_share: "395615871",
    },
    state: {
      total_bond_units: "224086887789575",
      total_debt_units: "380552330476190",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "690140721",
          debt_value: "341000000",
          ltv: "0.49410212964378898",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "415340521",
          debt_value: "58579946",
          ltv: "0.141040767847450164",
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
// total bond units     224086887789575
// total debt units     380552330476190
// user1 bond units     139895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399579947
// pool uANC            161019229
// pool uusd            973241418
// pool uLP             395615871
//
// We dump 35 ANC token in the AMM, which should barely make user1 liquidatable
// kValue = poolUst * poolAnc = 973241418 * 161019229
// = 156710582757226722
// returnUst = poolUst - k / (poolAnc + sendAnc)
// = 973241418 - 156710582757226722 / (161019229 + 100000000)
// = 372861962 uusd
// fee = returnUst * feeRate = 372861962 * 0.003
// = 1118585 uusd
// returnUstAfterFee = returnUst - fee = 372861962 - 1118585
// = 371743377 uusd
// returnUstAfterFeeAndTax = deductTax(returnUstAfterFee) = deductTax(371743377)
// = 371372004 uusd
// ustCostForPool = addTax(371372004) = 371743376 uusd
// ---
// pool uANC            161019229 + 100000000 = 261019229
// pool uusd            973241418 - 371743376 = 601498042
//
// Result
// ---
// total bond units     224086887789575
// total debt units     380552330476190
// user1 bond units     139895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399579947
// pool uANC            261019229
// pool uusd            601498042
// pool uLP             395615871
//
// ancPrice = computeSwapOutput(1000000, 261019229, 601498042) / 1000000
// = 2.295626 uusd per uANC
//
// State health:
// totalBondValue = (261019229 * 2.295626 + 601498042) * 225380740 / 395615871
// = 684034192 uusd
// totalDebtValue = 399579947
//
// User1 health:
// bondValue = 684034192 * 139895170000000 / 224086887789575 = 427035604
// debtValue = 399579947 * 324761904761905 / 380552330476190 = 341000000
// ltv = 341000000 / 427035604 = 0.798528265104564911
//
// User2 health:
// bondValue = 684034192 * 84191717789575 / 224086887789575 = 256998587
// debtValue = 399579947 * 55790425714285 / 380552330476190 = 58579946
// ltv = 58579946 / 256998587 = 0.227938786293793903
//--------------------------------------------------------------------------------------------------

async function testDump() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      send: {
        amount: "100000000",
        contract: astroport.pair.address,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  await verifier.verify(txhash, "testDump", {
    bond: {
      bond_amount: "225380740",
    },
    debt: {
      amount: "399579947",
    },
    pool: {
      assets: [
        // uusd
        { amount: "601498042" },
        // uANC
        { amount: "261019229" },
      ],
      total_share: "395615871",
    },
    state: {
      total_bond_units: "224086887789575",
      total_debt_units: "380552330476190",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "139895170000000",
          debt_units: "324761904761905",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "427035604",
          debt_value: "341000000",
          ltv: "0.798528265104564911",
        },
      },
      // user 2
      {
        address: user2.key.accAddress,
        position: {
          bond_units: "84191717789575",
          debt_units: "55790425714285",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "256998587",
          debt_value: "58579946",
          ltv: "0.227938786293793903",
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
// total bond units     224086887789575
// total debt units     380552330476190
// user1 bond units     139895170000000
// user1 debt units     324761904761905
// user1 unlocked uANC  0
// user1 unlocked uusd  1
// user1 unlocked uLP   0
// user2 bond units     84191717789575
// user2 debt units     55790425714285
// user2 unlocked uANC  0
// user2 unlocked uusd  1
// user2 unlocked uLP   0
// bond                 225380740
// debt                 399579947
// pool uANC            261019229
// pool uusd            601498042
// pool uLP             395615871
//
// Step 1. unbond
// reduce all of user1's 139895170000000 bond units
// amount to unbond: 225380740 * 139895170000000 / 224086887789575 = 140702908
// ---
// bond                 225380740 - 140702908 = 84677832
// total bond units     224086887789575 - 139895170000000 = 84191717789575
// user1 bond units     139895170000000 - 139895170000000 = 0
// user1 unlocked uLP   0 + 140702908 = 140702908
//
// Step 2. remove liquidity
// burn of of user1's 140702908 uLP
// ANC to be released: 261019229 * 140702908 / 395615871 = 92832889
// UST to be released: 601498042 * 140702908 / 395615871 = 213926007
// UST to receive: deductTax(213926007) = 213712294
// transaction cost for pool: addTax(213712294) = 213926006
// ---
// pool uANC            261019229 - 92832889 = 168186340
// pool uusd            601498042 - 213926006 = 387572036
// pool uLP             395615871 - 140702908 = 254912963
// user1 unlocked uANC  0 + 92832889 = 92832889
// user1 unlocked uusd  1 + 213712294 = 213712295
// user1 unlocked uLP   140702908 - 140702908 = 0
//
// Step 3. swap
// swap all of user1's 92832889 uANC for UST
// kValue = poolUst * poolAnc = 387572036 * 168186340
// = 65184322221188240
// returnUst = poolUst - k / (poolAnc + sendAnc)
// = 387572036 - 65184322221188240 / (168186340 + 92832889)
// = 137842074 uusd
// fee = returnUst * feeRate = 137842074 * 0.003
// = 413526 uusd
// returnUstAfterFee = returnUst - fee = 137842074 - 413526
// = 137428548 uusd
// returnUstAfterFeeAndTax = deductTax(137428548)
// = 137291256 uusd
// ustCostForPool = addTax(137291256) = 137428547 uusd
// ---
// pool uANC            168186340 + 92832889 = 261019229
// pool uusd            387572036 - 137428547 = 250143489
// user1 unlocked uANC  92832889 - 92832889 = 0
// user1 unlocked uusd  213712295 + 137291256 = 351003551
//
//
// Step 4. repay
// user1's debt amount: 341000000. repay this amount
// transaction cost: addTax(341000000) = 341341000
// user1's debt units is reduced to zero
// ---
// debt                 399579947 - 341000000 = 58579947
// total debt units     380552330476190 - 324761904761905 = 55790425714285
// user1 debt units     324761904761905 - 324761904761905 = 0
// user1 unlocked uusd  351003551 - 341341000 = 9662551
//
// Step 5. refund to liquidator
// uusdToRefund = 9662551 * bonusRate = 9662551 * 0.05 = 483127
// UST to send: deductTax(483127) = 482644
// transaction cost: addTax(482644) = 483126
// ---
// user1 unlocked uusd  9662551 - 483126 = 9179425
//
// Step 6. refund to user
// refund all the remaining unlocked uusd to user
// uusdToRefund = 9179425
// UST to send: deductTax(9179425) = 9170254
// transaction cost: addTax(9170254) = 9179424
// ---
// user1 unlocked uusd  9179425 - 9179424 = 1
//
// Result
// ---
// total bond units     84191717789575
// total debt units     55790425714285
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
// pool uANC            261019229
// pool uusd            250143489
// pool uLP             254912963
//
// ancPrice = computeSwapOutput(1000000, 261019229, 250143489) / 1000000
// = 0.954677 uusd per uANC
//
// State health:
// totalBondValue = (261019229 * 0.954677 + 250143489) * 84677832 / 254912963
// = 165869937 uusd
// totalDebtValue = 58579947
//
// User1 health:
// bondValue = 0
// debtValue = 0
// ltv = null
//
// User2 health:
// same as state as user2 is the only active position
// ltv = 58579947 / 165869937 = 0.353167958338345543
//--------------------------------------------------------------------------------------------------

async function testLiquidation() {
  const { txhash } = await sendTransaction(terra, liquidator, [
    new MsgExecuteContract(liquidator.key.accAddress, field.address, {
      liquidate: {
        user: user1.key.accAddress,
      },
    }),
  ]);

  await verifier.verify(txhash, "testLiquidation", {
    bond: {
      bond_amount: "84677832",
    },
    debt: {
      amount: "58579947",
    },
    pool: {
      assets: [
        // uusd
        { amount: "250143489" },
        // uANC
        { amount: "261019229" },
      ],
      total_share: "254912963",
    },
    state: {
      total_bond_units: "84191717789575",
      total_debt_units: "55790425714285",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
          ],
        },
        health: {
          bond_value: "165869937",
          debt_value: "58579947",
          ltv: "0.353167958338345543",
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
// total bond units     84191717789575
// total debt units     55790425714285
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
// pool uANC            261019229
// pool uusd            250143489
// pool uLP             254912963
//
// Step 1. unbond
// unbond all of user2's 84677832 uLP
// ---
// bond                 0
// total bond units     0
// user2 bond units     0
// user2 unlocked uLP   0 + 84677832 = 84677832
//
// Step 2. remove liquidity
// burn all of user2's 84677832 uLP
// ANC to be released: 261019229 * 84677832 / 254912963 = 86706231
// UST to be released: 250143489 * 84677832 / 254912963 = 83093492
// UST to receive: deductTax(83093492) = 83010481
// transaction cost for pool: addTax(83010481) = 83093491
// ---
// pool uANC            261019229 - 86706231 = 174312998
// pool uusd            250143489 - 83093491 = 167049998
// pool uLP             254912963 - 84677832 = 170235131
// user2 unlocked uANC  0 + 86706231 = 86706231
// user2 unlocked uusd  1 + 83010481 = 83010482
// user2 unlocked uLP   0
//
// Step 3. swap
// skipped as `swap_amount` is zero
//
// Step 4. repay
// user2's remaining debts: 58579947 uusd
// we try paying a bit more than that: 58600000
// transaction cost: addTax(58600000) = 58658600 uusd
// ---
// debt                 0
// total debt units     0
// user2 debt units     0
// user2 unlocked uusd  83010482 - 58658600 = 24351882
//
// Step 5. refund
// send all 24351882 uANC to user2
// UST to send: deductTax(24351882) = 24327554
// transaction cost: addTax(24327554) = 24351881
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
// pool uANC            174312998
// pool uusd            167049998
// pool uLP             170235131
//--------------------------------------------------------------------------------------------------

async function testReducePosition2() {
  const { txhash } = await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, field.address, {
      update_position: [
        {
          unbond: {
            bond_units_to_reduce: "84191717789575",
          },
        },
        {
          repay: {
            amount: "58600000",
          },
        },
      ],
    }),
  ]);

  await verifier.verify(txhash, "testReducePosition2", {
    bond: {
      bond_amount: "0",
    },
    debt: {
      amount: "0",
    },
    pool: {
      assets: [
        // uusd
        { amount: "167049998" },
        // uANC
        { amount: "174312998" },
      ],
      total_share: "170235131",
    },
    state: {
      total_bond_units: "0",
      total_debt_units: "0",
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "0",
          unlocked_assets: [
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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
            // uANC - 0
            // uusd
            { amount: "1" },
            // uLP - 0
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
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(treasury.key.accAddress)} as treasury`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);
  console.log(`Use ${chalk.cyan(liquidator.key.accAddress)} as liquidator`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Martian Field"));

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

  console.log(chalk.green("\nAll tests successfully completed. Hooray!\n"));
})();
