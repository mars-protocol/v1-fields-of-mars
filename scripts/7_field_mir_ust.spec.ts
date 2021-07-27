import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, toEncodedBinary } from "./helpers";
import {
  deployMartianField,
  deployMockMirror,
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
let mirrorToken: string;
let mirrorStaking: string;
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
    "Mock Mirror Token",
    "MIR"
  );
  mirrorToken = cw20Token;

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(
    terra,
    deployer,
    cw20CodeId,
    mirrorToken
  ));

  mirrorStaking = await deployMockMirror(
    terra,
    deployer,
    mirrorToken,
    mirrorToken,
    terraswapLpToken
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
      pair: terraswapPair,
      share_token: terraswapLpToken,
    },
    staking: {
      mirror: {
        contract_addr: mirrorStaking,
        asset_token: mirrorToken,
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

  process.stdout.write("Provide initial liquidity to TerraSwap Pair... ");

  // Deployer Provides 69 ANC + 420 UST
  // Should receive sqrt(69000000 * 420000000) = 170235131 uLP
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
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
// Tests
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
        bond_value: "838322513",
        debt_value: "420000000",
        ltv: "0.501000502177853357",
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
          bond_value: "838322513",
          debt_value: "420000000",
          ltv: "0.501000502177853357",
        },
      },
    ],
  });
}

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
        bond_value: "840733315",
        debt_value: "420000000",
        ltv: "0.49956388370312172",
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
          bond_value: "840733315",
          debt_value: "420000000",
          ltv: "0.49956388370312172",
        },
      },
    ],
  });
}

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
        bond_value: "840733315",
        debt_value: "441000000",
        ltv: "0.524542077888277806",
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
          bond_value: "840733315",
          debt_value: "441000000",
          ltv: "0.524542077888277806",
        },
      },
    ],
  });
}

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
        bond_value: "1257359536",
        debt_value: "499579947",
        ltv: "0.397324657503532068",
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
          bond_value: "840733317",
          debt_value: "441000000",
          ltv: "0.52454207664045744",
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
          bond_value: "416626218",
          debt_value: "58579946",
          ltv: "0.140605519933937522",
        },
      },
    ],
  });
}

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
        bond_value: "1257359536",
        debt_value: "399679847",
        ltv: "0.317872363120169631",
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
          bond_value: "840733317",
          debt_value: "341099900",
          ltv: "0.405717119927102877",
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
          bond_value: "416626218",
          debt_value: "58579946",
          ltv: "0.140605519933937522",
        },
      },
    ],
  });
}

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
        bond_value: "1108903290",
        debt_value: "399679847",
        ltv: "0.360428046885856024",
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
          bond_value: "692277070",
          debt_value: "341099900",
          ltv: "0.492721649729060071",
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
          bond_value: "416626219",
          debt_value: "58579946",
          ltv: "0.140605519596451513",
        },
      },
    ],
  });
}

async function testDump() {
  const { txhash } = await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
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
        { amount: "665352039" },
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
        bond_value: "704374722",
        debt_value: "399679847",
        ltv: "0.567425028918059328",
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
          bond_value: "439733990",
          debt_value: "341099900",
          ltv: "0.775696006578886476",
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
          bond_value: "264640731",
          debt_value: "58579946",
          ltv: "0.221356500107309634",
        },
      },
    ],
  });
}

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
        { amount: "80351828" },
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
        total_debt_units: "76525550476190",
      },
      health: {
        bond_value: "264640734",
        debt_value: "80351828",
        ltv: "0.303626077457901851",
      },
    },
    users: [
      // user 1
      {
        address: user1.key.accAddress,
        position: {
          bond_units: "0",
          debt_units: "20735124761905",
          unlocked_assets: [
            // uMIR
            { amount: "16160455" },
            // uusd
            { amount: "1" },
            // uLP
            { amount: "0" },
          ],
        },
        health: {
          bond_value: "0",
          debt_value: "21771881",
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
          bond_value: "264640734",
          debt_value: "58579946",
          ltv: "0.221356497597985047",
        },
      },
    ],
  });
}

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
        bond_value: "264640734",
        debt_value: "58579947",
        ltv: "0.221356501376692826",
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
          bond_value: "264640734",
          debt_value: "58579947",
          ltv: "0.221356501376692826",
        },
      },
    ],
  });
}

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
