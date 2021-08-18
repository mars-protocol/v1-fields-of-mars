import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, toEncodedBinary } from "./helpers";
import {
  deployMartianField,
  deployMockMirror,
  deployMockMars,
  deployAstroportPair,
  deployAstroportToken,
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
let mirrorToken: string;
let mirrorStaking: string;
let astroportPair: string;
let astroportLpToken: string;
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
  let { cw20CodeId, cw20Token } = await deployAstroportToken(
    terra,
    deployer,
    "Mock Mirror Token",
    "MIR"
  );
  mirrorToken = cw20Token;

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(terra, deployer, {
    asset_infos: [
      { native_token: { denom: "uusd" } },
      { token: { contract_addr: mirrorToken } },
    ],
    token_code_id: cw20CodeId,
  }));

  mirrorStaking = await deployMockMirror(
    terra,
    deployer,
    mirrorToken,
    mirrorToken,
    astroportLpToken
  );

  redBank = await deployMockMars(terra, deployer);

  // Part 2. Deploy Martian Field
  config = {
    long_asset: {
      token: {
        contract_addr: mirrorToken,
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
      pair: astroportPair,
      share_token: astroportLpToken,
    },
    staking: {
      mirror: {
        contract_addr: mirrorStaking,
        asset_token: mirrorToken,
        staking_token: astroportLpToken,
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
  process.stdout.write("Fund deployer with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000", // 1000 MIR
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user1 with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user2 with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "34500000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Mirror Staking contract with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: mirrorStaking,
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

  process.stdout.write("Provide initial liquidity to Astroport Pair... ");

  // Deployer Provides 69 ANC + 420 UST
  // Should receive sqrt(69000000 * 420000000) = 170235131 uLP
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: astroportPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      astroportPair,
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
                  contract_addr: mirrorToken,
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
        // uMIR
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
//----------------------------------------------------------------------------------------

async function testOpenPosition1() {
  const { txhash } = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
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
                contract_addr: mirrorToken,
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
        // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
//----------------------------------------------------------------------------------------

async function testOpenPosition2() {
  const { txhash } = await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, mirrorToken, {
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
                  contract_addr: mirrorToken,
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
//----------------------------------------------------------------------------------------

async function testDump() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      send: {
        amount: "100000000",
        contract: astroportPair,
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
            // uMIR
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
        // uMIR
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
            // uMIR
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
            // uMIR
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

  console.log(chalk.yellow("\nTest: Martian Field: MIR-UST LP"));

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
