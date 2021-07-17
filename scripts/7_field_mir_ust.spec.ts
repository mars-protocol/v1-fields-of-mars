import * as path from "path";
import BN, { red } from "bn.js";
import chalk from "chalk";
import chai from "chai";
import chaiAsPromised from "chai-as-promised";
import {
  LocalTerra,
  MsgExecuteContract,
  MsgMigrateContract,
  MsgSend,
} from "@terra-money/terra.js";
import {
  deployMartianField,
  deployMockMars,
  deployMockMirror,
  deployTerraswapPair,
  deployTerraswapToken,
} from "./fixture";
import {
  GAS_AMOUNT,
  queryNativeTokenBalance,
  queryTokenBalance,
  sendTransaction,
  storeCode,
  toEncodedBinary,
} from "./helpers";
import { Verifier } from "./verifier";

chai.use(chaiAsPromised);
const { expect } = chai;

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const treasury = terra.wallets.test2;
const user1 = terra.wallets.test3;
const user2 = terra.wallets.test4;
const liquidator1 = terra.wallets.test5;
const liquidator2 = terra.wallets.test6;

let mirrorToken: string;
let mirrorStaking: string;
let terraswapPair: string;
let terraswapLpToken: string;
let redBank: string;
let strategy: string;
let verifier: Verifier;

//----------------------------------------------------------------------------------------
// SETUP
//----------------------------------------------------------------------------------------

async function setupTest() {
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

  strategy = await deployMartianField(
    terra,
    deployer,
    "../artifacts/martian_field.wasm",
    {
      owner: deployer.key.accAddress,
      operators: [user1.key.accAddress],
      treasury: treasury.key.accAddress,
      asset_token: mirrorToken,
      reward_token: mirrorToken,
      pool: terraswapPair,
      pool_token: terraswapLpToken,
      red_bank: {
        contract_addr: redBank,
        borrow_asset: {
          native_token: {
            denom: "uusd",
          },
        },
      },
      staking: {
        mirror: {
          contract_addr: mirrorStaking,
          asset_token: mirrorToken,
          staking_token: terraswapLpToken,
        },
      },
      max_ltv: "0.67", // 67% debt ratio, i.e. 150% collateralization ratio
      performance_fee_rate: "0.20", // 20%
      liquidation_fee_rate: "0.05", // 5%
    }
  );

  process.stdout.write("Creating verifier object... ");

  verifier = new Verifier(terra, {
    strategy,
    redBank,
    assetToken: mirrorToken,
    staking: mirrorStaking,
  });

  console.log(chalk.green("Done!"));

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
}

//----------------------------------------------------------------------------------------
// TEST CONFIG
//----------------------------------------------------------------------------------------

async function testConfig() {
  process.stdout.write("Should store correct config info... ");

  await verifier.verifyConfig({
    owner: deployer.key.accAddress,
    operators: [user1.key.accAddress],
    treasury: treasury.key.accAddress,
    asset_token: mirrorToken,
    reward_token: mirrorToken,
    pool: terraswapPair,
    pool_token: terraswapLpToken,
    red_bank: {
      contract_addr: redBank,
      borrow_asset: {
        native_token: {
          denom: "uusd",
        },
      },
    },
    staking: {
      mirror: {
        contract_addr: mirrorStaking,
        asset_token: mirrorToken,
        staking_token: terraswapLpToken,
      },
    },
    max_ltv: "0.67",
    performance_fee_rate: "0.2",
    liquidation_fee_rate: "0.05",
  });
  await verifier.verifyState({
    total_bond_value: "0",
    total_bond_units: "0",
    total_debt_value: "0",
    total_debt_units: "0",
    ltv: null,
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST OPEN POSITION 1
//----------------------------------------------------------------------------------------

async function testOpenPosition1() {
  process.stdout.write("Should open position for user 1... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, mirrorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: strategy,
      },
    }),
    new MsgExecuteContract(user1.key.accAddress, strategy, {
      increase_position: {
        asset_amount: "69000000",
      },
    }),
  ]);

  // See `6_strategy_anchor.spec.ts` for the calculation of these expected outputs
  await verifier.verifyState({
    total_bond_value: "838322513",
    total_bond_units: "169895170000000",
    total_debt_value: "420000000",
    total_debt_units: "420000000000000",
    ltv: "0.501000502177853357",
  });

  const expectedPosition = {
    is_active: true,
    bond_value: "838322513",
    bond_units: "169895170000000",
    debt_value: "420000000",
    debt_units: "420000000000000",
    ltv: "0.501000502177853357",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  };
  await verifier.verifyPosition(user1, expectedPosition);
  await verifier.verifyPositionSnapshot(user1, expectedPosition);

  await verifier.verifyDebt("uusd", "420000000");
  await verifier.verifyBondInfo("mirror", "169895170");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST HARVEST
//----------------------------------------------------------------------------------------

async function testHarvest() {
  process.stdout.write("Should harvest staking rewards... ");

  // Should fail as user2 is not a whitelisted operator
  await expect(
    sendTransaction(terra, user2, [
      new MsgExecuteContract(user2.key.accAddress, strategy, {
        harvest: {},
      }),
    ])
  ).to.be.rejectedWith("unauthorized");

  // User1 is a whitelisted operator; this should work
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, strategy, {
      harvest: {},
    }),
  ]);

  await verifier.verifyState({
    total_bond_value: "840733315",
    total_bond_units: "169895170000000",
    total_debt_value: "420000000",
    total_debt_units: "420000000000000",
    ltv: "0.49956388370312172",
  });
  await verifier.verifyPosition(user1, {
    is_active: true,
    bond_value: "840733315",
    bond_units: "169895170000000",
    debt_value: "420000000",
    debt_units: "420000000000000",
    ltv: "0.49956388370312172",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyDebt("uusd", "420000000");
  await verifier.verifyBondInfo("mirror", "170876125");

  // Although the position is changed, the snapshot should have not changed
  await verifier.verifyPositionSnapshot(user1, {
    is_active: true,
    bond_value: "838322513",
    bond_units: "169895170000000",
    debt_value: "420000000",
    debt_units: "420000000000000",
    ltv: "0.501000502177853357",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });

  // Fee collector should have received 0.2 MIR performance fee
  const treasuryBalance = await queryTokenBalance(
    terra,
    treasury.key.accAddress,
    mirrorToken
  );
  expect(treasuryBalance).to.equal("200000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST OPEN POSITION 2
//----------------------------------------------------------------------------------------

async function testOpenPosition2() {
  process.stdout.write("Should open position for user 2... ");

  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, mirrorToken, {
      increase_allowance: {
        amount: "34500000",
        spender: strategy,
      },
    }),
    new MsgExecuteContract(
      user2.key.accAddress,
      strategy,
      {
        increase_position: {
          asset_amount: "34500000",
        },
      },
      {
        uusd: "208579947",
      }
    ),
  ]);

  await verifier.verifyState({
    total_bond_value: "1257476465",
    total_bond_units: "254110517355205",
    total_debt_value: "420000000",
    total_debt_units: "420000000000000",
    ltv: "0.334002274945161697",
  });
  await verifier.verifyPosition(user1, {
    is_active: true,
    bond_value: "840733316",
    bond_units: "169895170000000",
    debt_value: "420000000",
    debt_units: "420000000000000",
    ltv: "0.499563883108921545",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyPosition(user2, {
    is_active: true,
    bond_value: "416743144",
    bond_units: "84215347355205",
    debt_value: "0",
    debt_units: "0",
    ltv: "0",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyDebt("uusd", "420000000");
  await verifier.verifyBondInfo("mirror", "255577722");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST PAY DEBT
//----------------------------------------------------------------------------------------

async function testPayDebt() {
  process.stdout.write("Should repaying debt... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      strategy,
      {
        pay_debt: {
          user: user1.key.accAddress,
        },
      },
      {
        uusd: "100000000",
      }
    ),
  ]);

  await verifier.verifyState({
    total_bond_value: "1257476465",
    total_bond_units: "254110517355205",
    total_debt_value: "320099901",
    total_debt_units: "320099901000000",
    ltv: "0.254557369389811999",
  });
  await verifier.verifyPosition(user1, {
    is_active: true,
    bond_value: "840733316",
    bond_units: "169895170000000",
    debt_value: "320099901",
    debt_units: "320099901000000",
    ltv: "0.380738927443669902",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyPosition(user2, {
    is_active: true,
    bond_value: "416743144",
    bond_units: "84215347355205",
    debt_value: "0",
    debt_units: "0",
    ltv: "0",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyDebt("uusd", "320099901");
  await verifier.verifyBondInfo("mirror", "255577722");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST REDUCE POSITION
//----------------------------------------------------------------------------------------

async function testReducePosition() {
  process.stdout.write("Should reduce position... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, strategy, {
      reduce_position: {
        bond_units: "69895170000000",
      },
    }),
  ]);

  await verifier.verifyState({
    total_bond_value: "911597314",
    total_bond_units: "184215347355205",
    total_debt_value: "147505687",
    total_debt_units: "147505687000000",
    ltv: "0.161810137803894406",
  });
  await verifier.verifyPosition(user1, {
    is_active: true,
    bond_value: "494854158",
    bond_units: "100000000000000",
    debt_value: "147505687",
    debt_units: "147505687000000",
    ltv: "0.298079110007195291",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyPosition(user2, {
    is_active: true,
    bond_value: "416743150",
    bond_units: "84215347355205",
    debt_value: "0",
    debt_units: "0",
    ltv: "0",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyBondInfo("mirror", "185278986");
  await verifier.verifyDebt("uusd", "147505687");

  // User 1 should have received 28610622 uMIR
  const user1MirBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    mirrorToken
  );
  expect(user1MirBalance).to.equal("28610622");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST LIQUIDATION 1 - close position + incomplete claim of collateral
//----------------------------------------------------------------------------------------

async function testLiquidation1() {
  process.stdout.write("Should partially liquidate user 1... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      send: {
        amount: "500000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  await verifier.verifyState({
    total_bond_value: "206713261",
    total_bond_units: "184215347355205",
    total_debt_value: "147505687",
    total_debt_units: "147505687000000",
    ltv: "0.713576314777405596",
  });
  await verifier.verifyPosition(user1, {
    is_active: true,
    bond_value: "112212833",
    bond_units: "100000000000000",
    debt_value: "147505687",
    debt_units: "147505687000000",
    ltv: "1.314517092710777563",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyPosition(user2, {
    is_active: true,
    bond_value: "94500427",
    bond_units: "84215347355205",
    debt_value: "0",
    debt_units: "0",
    ltv: "0",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyBondInfo("mirror", "185278986");
  await verifier.verifyDebt("uusd", "147505687");

  await sendTransaction(terra, liquidator1, [
    new MsgExecuteContract(
      liquidator1.key.accAddress,
      strategy,
      {
        liquidate: {
          user: user1.key.accAddress,
        },
      },
      {
        uusd: 50000000,
      }
    ),
  ]);

  await verifier.verifyState({
    total_bond_value: "94500429",
    total_bond_units: "84215347355205",
    total_debt_value: "41561268",
    total_debt_units: "41561268000000",
    ltv: "0.439799781226389988",
  });
  await verifier.verifyPosition(user1, {
    is_active: false,
    bond_value: "0",
    bond_units: "0",
    debt_value: "41561268",
    debt_units: "41561268000000",
    ltv: null, // Option<T>::None is serialized as null
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "82833879",
  });
  await verifier.verifyPosition(user2, {
    is_active: true,
    bond_value: "94500429",
    bond_units: "84215347355205",
    debt_value: "0",
    debt_units: "0",
    ltv: "0",
    unbonded_ust_amount: "0",
    unbonded_asset_amount: "0",
  });
  await verifier.verifyBondInfo("mirror", "84701598");
  await verifier.verifyDebt("uusd", "41561268");

  // Liquidator should have receive correct amount of MIR token
  const liquidatorMirBalance = await queryTokenBalance(
    terra,
    liquidator1.key.accAddress,
    mirrorToken
  );
  expect(liquidatorMirBalance).to.equal("99553178");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST LIQUIDATION 2 - complete claim of collateral
//----------------------------------------------------------------------------------------

async function testLiquidation2() {
  process.stdout.write("Should completely liquidate user 1... ");

  // User 1 has 41561268 uusd debt remaining. To fully pay down these, liquidator needs:
  // deduct(x) = 41561268 => x = 41602830
  await sendTransaction(terra, liquidator2, [
    new MsgExecuteContract(
      liquidator2.key.accAddress,
      strategy,
      {
        liquidate: {
          user: user1.key.accAddress,
        },
      },
      {
        uusd: 41602830,
      }
    ),
  ]);

  await verifier.verifyState({
    total_bond_value: "94500429",
    total_bond_units: "84215347355205",
    total_debt_value: "0",
    total_debt_units: "0",
    ltv: "0",
  });
  await verifier.verifyDebt("uusd", "0");

  // User 1 is fully liquidated, so his position data should have been purged from storage
  // Querying it should fail with statue code 500
  await expect(verifier.verifyPosition(user1, {})).to.be.rejectedWith("status code 500");

  // Same with the position snapshot
  await expect(verifier.verifyPositionSnapshot(user1, {})).to.be.rejectedWith(
    "status code 500"
  );

  // Liquidator should have received all of user 1's unstaked MIR, which is 82833879
  const liquidatorMirBalance = await queryTokenBalance(
    terra,
    liquidator2.key.accAddress,
    mirrorToken
  );
  expect(liquidatorMirBalance).to.equal("82833879");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST CLOSE POSITION
//----------------------------------------------------------------------------------------

async function testClosePosition() {
  process.stdout.write("Should close position... ");

  const userUstBalanceBefore = await queryNativeTokenBalance(
    terra,
    user2.key.accAddress,
    "uusd"
  );

  // User 2 closes position completely and withdraw all assets by not providing optional
  // `bond_units` argument
  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, strategy, {
      reduce_position: {},
    }),
  ]);

  // All assets and debts should have been removed
  await verifier.verifyState({
    total_bond_value: "0",
    total_bond_units: "0",
    total_debt_value: "0",
    total_debt_units: "0",
    ltv: null,
  });

  // User's position as well as the snapshot should have been deleted
  await expect(verifier.verifyPosition(user2, {})).to.be.rejectedWith("status code 500");
  await expect(verifier.verifyPositionSnapshot(user2, {})).to.be.rejectedWith(
    "status code 500"
  );

  const userMirBalance = await queryTokenBalance(
    terra,
    user2.key.accAddress,
    mirrorToken
  );
  expect(userMirBalance).to.equal("153597896");

  // Note: Must use bn.js here for the calculation, because the UST balances may be out of
  // bond for the native Javascript integer type
  const userUstBalanceAfter = await queryNativeTokenBalance(
    terra,
    user2.key.accAddress,
    "uusd"
  );
  expect(
    new BN(userUstBalanceAfter).sub(new BN(userUstBalanceBefore)).toNumber()
  ).to.be.equal(47155854 - GAS_AMOUNT);

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST UPDATE CONFIG
//----------------------------------------------------------------------------------------

async function testUpdateConfig() {
  process.stdout.write("Should update config... ");

  const executeMsg = {
    update_config: {
      new_config: {
        owner: deployer.key.accAddress,
        operators: [],
        treasury: treasury.key.accAddress,
        asset_token: mirrorToken,
        reward_token: mirrorToken,
        mirror_staking: mirrorStaking,
        pool: terraswapPair,
        pool_token: terraswapLpToken,
        red_bank: {
          contract_addr: redBank,
          borrow_asset: {
            native_token: {
              denom: "uusd",
            },
          },
        },
        staking: {
          mirror: {
            contract_addr: mirrorStaking,
            asset_token: mirrorToken,
            staking_token: terraswapLpToken,
          },
        },
        max_ltv: "0.67",
        performance_fee_rate: "1.00", // used to be 20%; try updating this to 100%
        liquidation_fee_rate: "0.05",
      },
    },
  };

  // Try updating config with a non-owner user; should fail
  await expect(
    sendTransaction(terra, user1, [
      new MsgExecuteContract(user1.key.accAddress, strategy, executeMsg),
    ])
  ).to.be.rejectedWith("unauthorized");

  // Try updating with owner account; should succeed
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, strategy, executeMsg),
  ]);

  await verifier.verifyConfig({
    owner: deployer.key.accAddress,
    operators: [],
    treasury: treasury.key.accAddress,
    asset_token: mirrorToken,
    reward_token: mirrorToken,
    pool: terraswapPair,
    pool_token: terraswapLpToken,
    red_bank: {
      contract_addr: redBank,
      borrow_asset: {
        native_token: {
          denom: "uusd",
        },
      },
    },
    staking: {
      mirror: {
        contract_addr: mirrorStaking,
        asset_token: mirrorToken,
        staking_token: terraswapLpToken,
      },
    },
    max_ltv: "0.67",
    performance_fee_rate: "1", // should correctly update to 100%
    liquidation_fee_rate: "0.05",
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST MIGRATION
//----------------------------------------------------------------------------------------

async function testMigrate() {
  process.stdout.write("Should migrate... ");

  // Upload another copy of the contract code, get codeId
  const newCodeId = await storeCode(
    terra,
    deployer,
    path.resolve(__dirname, "../artifacts/martian_field.wasm")
  );

  // Try migrate with a non-owner user; should fail
  await expect(
    sendTransaction(terra, user1, [
      new MsgMigrateContract(user1.key.accAddress, strategy, newCodeId, {}),
    ])
  ).to.be.rejectedWith("unauthorized");

  await expect(
    sendTransaction(terra, deployer, [
      new MsgMigrateContract(deployer.key.accAddress, strategy, newCodeId, {}),
    ])
  ).to.be.rejectedWith("unimplemented");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// MAIN
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(treasury.key.accAddress)} as treasury`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Strategy: MIR-UST LP"));

  await testConfig();
  await testOpenPosition1();
  await testHarvest();
  await testOpenPosition2();
  await testPayDebt();
  await testReducePosition();
  await testLiquidation1();
  await testLiquidation2();
  await testClosePosition();
  await testUpdateConfig();
  await testMigrate();

  console.log("");
})();
