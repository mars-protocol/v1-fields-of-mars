import * as path from "path";
import BN from "bn.js";
import chalk from "chalk";
import chai from "chai";
import chaiAsPromised from "chai-as-promised";
import {
  LocalTerra,
  MsgExecuteContract,
  MsgMigrateContract,
  MsgSend,
  Wallet,
} from "@terra-money/terra.js";
import {
  deployMartianField,
  deployMockAnchor,
  deployMockMars,
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

chai.use(chaiAsPromised);
const { expect } = chai;

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const treasury = terra.wallets.test2;
const user1 = terra.wallets.test3;
const user2 = terra.wallets.test4;
const liquidator1 = terra.wallets.test5;
const liquidator2 = terra.wallets.test6;

let anchorToken: string;
let anchorStaking: string;
let terraswapPair: string;
let terraswapLpToken: string;
let redBank: string;
let field: string;

let config: object;
let unlockedAssets: object;

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

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(
    terra,
    deployer,
    cw20CodeId,
    anchorToken
  ));

  anchorStaking = await deployMockAnchor(terra, deployer, anchorToken, terraswapLpToken);

  redBank = await deployMockMars(terra, deployer);

  // Part 2. Initialize helper objects (helps reducing repetitive code in tests)
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
    keepers: [user1.key.accAddress],
    treasury: treasury.key.accAddress,
    governance: deployer.key.accAddress,
    max_ltv: "0.67", // 67% debt ratio, i.e. 150% collateralization ratio
    fee_rate: "0.2", // 20%
  };

  unlockedAssets = [
    {
      info: {
        token: {
          contract_addr: anchorToken,
        },
      },
      amount: "0",
    },
    {
      info: {
        native_token: {
          denom: "uusd",
        },
      },
      amount: "0",
    },
    {
      info: {
        token: {
          contract_addr: terraswapLpToken,
        },
      },
      amount: "0",
    },
  ];

  // Part 3. Deploy Martian Field
  field = await deployMartianField(
    terra,
    deployer,
    "../artifacts/martian_field.wasm",
    config
  );

  // Part 4. Misc
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
  // Should receive sqrt(69 * 420) = 170.235131 LP tokens
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
}

//----------------------------------------------------------------------------------------
// Test: Config
//----------------------------------------------------------------------------------------

async function testConfig() {
  process.stdout.write("Should store correct config info... ");

  await verifyConfig(config);

  // No position has been opened; State should be zero
  await verifyState({
    total_bond_units: "0",
    total_debt_units: "0",
  });
  await verifyHealth(null, {
    bond_value: "0",
    debt_value: "0",
    ltv: null,
  });

  // User1's position should be zero; query Position or Health or Snapshot should fail
  await expect(verifyPosition(user1, {})).to.be.rejectedWith("status code 500");
  await expect(verifyHealth(user1, {})).to.be.rejectedWith("status code 500");
  await expect(verifySnapshot(user1, {}, {})).to.be.rejectedWith("status code 500");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Open Position, Pt. 1
//
// User attempts to borrow 420 UST; should get 419.580419 after tax
// When providing to Terraswap, deliverable is 419.161257 after rax
//
// Terraswap currently has 69 ANC + 420 UST, with
// sqrt(69_000_000 * 420_000_000) = 170235131 total shares
//
// User provides 69 ANC + 419.161257 UST, should get this many LP tokens:
//
// min(ustDeposit * totalShare / ustPooled, ancDeposit * totalShare / ancPooled)
// = 419161257 * 170235131 / 420000000
// = 169895170
//
// Initial ratio 1 LP token = 1_000_000 asset units
// The user should have:
// 169895170000000 asset units
// 420000000000000 debt units
//
// After liquidity provision, the Terraswap pair has:
// 69 + 69 = 138 ANC
// 420000000 + 419161257 = 839161257 uusd
// 170235131 + 169895170 = 340130301 total shares
//
//------------------------------- DEBT RATIO CALCULATION ---------------------------------
//
// Value per LP token:
// 2 * 839161257 / 340130301 = 4.93435165601432258 uusd/uLP
//
// The value of the user's staked asset is
// 169895170 * 4.93435165601432258 = 838322513 uusd
// User's debt ratio is:
// 420000000 / 838322513 = 0.501000502177853357
//----------------------------------------------------------------------------------------

async function testOpenPosition1() {
  process.stdout.write("Should open position for user 1... ");

  await sendTransaction(terra, user1, [
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

  // State
  await verifyState({
    total_bond_units: "169895170000000",
    total_debt_units: "420000000000000",
  });
  await verifyHealth(null, {
    bond_value: "838322513",
    debt_value: "420000000",
    ltv: "0.501000502177853357",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "169895170000000",
    debt_units: "420000000000000",
    unlocked_assets: unlockedAssets,
  });
  await verifySnapshot(
    user1,
    {
      is_active: true,
      bond_units: "169895170000000",
      debt_units: "420000000000000",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "838322513",
      debt_value: "420000000",
      ltv: "0.501000502177853357",
    }
  );

  // Bond & debt amounts
  await verifyBond("169895170");
  await verifyDebt("420000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Harvest
//
// 1.0 ANC reward claimed, 0.2 ANC charged as performance fee, 0.4 ANC swapped for UST
//
// Prior to the swap, Terraswap pool has:
// 138 ANC
// 4839161257 uusd
// 340130301 total shares
//
//-------------------------------- STEP 1. SWAP REWARD -----------------------------------
//
// If swapping 0.4 ANC to UST, the deliverable amount is calculated as follows:
//
// kValue = poolUst * poolAnc = 839161257 * 138000000
// = 115804253466000000
//
// returnUst = poolUst - k / (poolAnc + sendAnc)
// = 839161257 - 115804253466000000 / (138000000 + 400000)
// = 2425321 uusd
//
// fee = returnUst * feeRate = 2425321 * 0.003
// = 7275 uusd
//
// returnUstAfterFee = returnUst - fee = 2425321 - 7275
// = 2418046 uusd
//
// returnUstAfterFeeAndTax = deductTax(returnUstAfterFee) = deductTax(2418046)
// = 2415630 uusd
//
// The pool should now have
// 138 + 0.4 = 138.4 ANC
// 839161257 - 2418046 = 836743211 uusd
// 340130301 total shares (unchanged)
//
//----------------------------- STEP 2. PROVIDE LIQUIDITY --------------------------------
//
// When providing liquidity, the deliverable amount is
// deductTax(2415630) = 2413216 uusd
//
// Providing 0.4 ANC + 2413216 uusd, should get this many LP tokens:
// min(2413216 * 340130301 / 836743211, 400000 * 340130130 / 138400000)
// = 980955
//
// The pool should now have
// 138.4 + 0.4 = 138.8 ANC
// 836743211 + 2413216 = 839156427 uusd
// 340130301 + 980955 = 341111256 total shares
//
// Total bond amount 169895170 + 980955 = 170876125
//
//------------------------------- DEBT RATIO CALCULATION ---------------------------------
//
// Value per LP token:
// 2 * 839156427 / 341111256 = 4.92013331275119223 uusd/uLP
//
// Asset value = 170876125 * 4.920133312751192238 = 840733315 uusd
// Debt value = 420000000 uusd (unchanged)
//
// Debt ratio = 420000000 / 840733314 = 0.499563883703121720 (last digit 0 is dropped)
//----------------------------------------------------------------------------------------

async function testHarvest() {
  process.stdout.write("Should harvest staking rewards... ");

  // Should fail as user2 is not a whitelisted operator
  await expect(
    sendTransaction(terra, user2, [
      new MsgExecuteContract(user2.key.accAddress, field, {
        harvest: {},
      }),
    ])
  ).to.be.rejectedWith("unauthorized");

  // User1 is a whitelisted operator; this should work
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, field, {
      harvest: {},
    }),
  ]);

  // State
  await verifyState({
    total_bond_units: "169895170000000",
    total_debt_units: "420000000000000",
  });
  await verifyHealth(null, {
    bond_value: "840733315",
    debt_value: "420000000",
    ltv: "0.49956388370312172",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "169895170000000",
    debt_units: "420000000000000",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user1, {
    bond_value: "840733315",
    debt_value: "420000000",
    ltv: "0.49956388370312172",
  });
  await verifySnapshot(
    user1,
    {
      is_active: true,
      bond_units: "169895170000000",
      debt_units: "420000000000000",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "838322513",
      debt_value: "420000000",
      ltv: "0.501000502177853357",
    }
  );

  // Bond & debt amounts
  await verifyBond("170876125");
  await verifyDebt("420000000");

  // Fee collector should have received 0.2 ANC performance fee
  const treasuryBalance = await queryTokenBalance(
    terra,
    treasury.key.accAddress,
    anchorToken
  );
  expect(treasuryBalance).to.equal("200000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Open Position, Pt. 2
//
// User 2 sends enough UST along with the transaction so that he doesn't need to take on
// debt from Mars.
//
// The pool currently has:
// 138.8 ANC
// 839156427 uusd
// 341111256 total shares
//
// To pair with 34.5 ANC, this much uusd is needed:
// 839156427 * 34500000 / 138800000 = 208579947 uusd
//
// The actual amount deliverable to Terraswap is
// deductTax(208579947) = 208371575 uusd
//
// User 2 should receive this many LP tokens:
// min(208371575 * 341111256 / 839156427, 34500000 * 341111256 / 138800000)
// = min(84701597, 84786299)
// = 84701597
//
// Pooled assets after provision:
// 138.8 + 34.5 = 173.3 ANC
// 839156427 + 208371575 = 1047528002 uusd
// 341111256 + 84701597 = 425812853 total shares
//
// User 2 should be accredit this many asset units:
// totalAssetUnits * lpTokensAdded / totalLpTokens
// = 169895170000000 * 84701597 / 170876125
// = 84215347355205
//
// State after position increase:
// totalAssetUnits = 169895170000000 + 84215347355205 = 254110517355205
// totalDebtUnits = 420000000000000 + 0 = 420000000000000
// totalLpTokensStaked = 170876125 + 84701597 = 255577722
//
//------------------------------- DEBT RATIO CALCULATION ---------------------------------
//
// Value per LP token:
// 2 * 1047528002 / 425812853 = 4.920133314998831188 uusd/uLP
//
// Strategy:
// Asset value = 255577722 * 4.920133314998831188 = 1257476465 uusd
// Debt value = 420000000 uusd (unchanged)
// Debt ratio = 420000000 / 1257476465 = 0.334002274945161697
//
// User 1:
// LP tokens staked = 255577722 * 169895170000000 / 254110517355205 = 170876125 uLP
// Asset value = 170876125 * 4.920133314998831188 = 840733316 uusd
// Debt value = 420000000 uusd (unchanged)
// Debt ratio = 420000000 / 840733316 = 0.499563883108921545
//
// User 2:
// LP tokens staked = 255577722 * 84215347355205 / 254110517355205 = 84701596 uLP
// Asset value = 84701596 * 4.920133314998831188 = 416743144 uusd
// Debt value = 0
// Debt ratio = 0
//----------------------------------------------------------------------------------------

async function testOpenPosition2() {
  process.stdout.write("Should open position for user 2... ");

  await sendTransaction(terra, user2, [
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
              amount: "208579947",
            },
          ],
        },
      },
      {
        uusd: "208579947",
      }
    ),
  ]);

  // State
  await verifyState({
    total_bond_units: "254110517355205",
    total_debt_units: "420000000000000",
  });
  await verifyHealth(null, {
    bond_value: "1257476465",
    debt_value: "420000000",
    ltv: "0.334002274945161697",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "169895170000000",
    debt_units: "420000000000000",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user1, {
    bond_value: "840733316",
    debt_value: "420000000",
    ltv: "0.499563883108921545",
  });
  await verifySnapshot(
    user1,
    {
      is_active: true,
      bond_units: "169895170000000",
      debt_units: "420000000000000",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "838322513",
      debt_value: "420000000",
      ltv: "0.501000502177853357",
    }
  );

  // User 2
  await verifyPosition(user2, {
    is_active: true,
    bond_units: "84215347355205",
    debt_units: "0",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user2, {
    bond_value: "416743144",
    debt_value: "0",
    ltv: "0",
  });
  await verifySnapshot(
    user2,
    {
      is_active: true,
      bond_units: "84215347355205",
      debt_units: "0",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "416743144",
      debt_value: "0",
      ltv: "0",
    }
  );

  // Bond & debt amounts
  await verifyBond("255577722");
  await verifyDebt("420000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Pay Debt
//
// User 1 pays 100 UST
//
// After tax, the deliverable amount is deductTax(100_000_000) = 99900099 uusd
// The current debt amount should be 420_000_000 - 99900099 = 320099901 uusd
//
// The user's debt units should be reduced accordingly to 320099901000000
//
//------------------------------- DEBT RATIO CALCULATION -------------------------------
//
// Strategy:
// Asset value = 1257476465 uusd (unchanged)
// Debt value = 320099901 uusd
// Debt ratio = 320099901 / 1257476465 = 0.254557369389811999
//
// User 1:
// Asset value = 840733316 uusd (unchanged)
// Debt value = 320099901 uusd (unchanged)
// Debt ratio = 320099901 / 840733316 = 0.380738927443669902
//----------------------------------------------------------------------------------------

async function testPayDebt() {
  process.stdout.write("Should repaying debt... ");

  await sendTransaction(terra, user1, [
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

  // State
  await verifyState({
    total_bond_units: "254110517355205",
    total_debt_units: "320099901000000",
  });
  await verifyHealth(null, {
    bond_value: "1257476465",
    debt_value: "320099901",
    ltv: "0.254557369389811999",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "169895170000000",
    debt_units: "320099901000000",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user1, {
    bond_value: "840733316",
    debt_value: "320099901",
    ltv: "0.380738927443669902",
  });
  await verifySnapshot(
    user1,
    {
      is_active: true,
      bond_units: "169895170000000",
      debt_units: "320099901000000",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "840733316",
      debt_value: "320099901",
      ltv: "0.380738927443669902",
    }
  );

  // User 2: should not have changed from the last test
  await verifyPosition(user2, {
    is_active: true,
    bond_units: "84215347355205",
    debt_units: "0",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user2, {
    bond_value: "416743144",
    debt_value: "0",
    ltv: "0",
  });
  await verifySnapshot(
    user2,
    {
      is_active: true,
      bond_units: "84215347355205",
      debt_units: "0",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "416743144",
      debt_value: "0",
      ltv: "0",
    }
  );

  // Bond & debt amounts
  await verifyBond("255577722");
  await verifyDebt("320099901");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Reduce Position, Pt. 1
//
// User 1 has 169895170000000 asset units; we attempt to unstake 69895170000000
//
// Currently the strategy has 255.577722 LP tokens staked, with 254110517355205 total
// asset units
//
// The amount of LP tokens to be unbonded:
// 255577722 * 69895170000000 / 254110517355205 = 70298736
//
// The remaining bonded amount is
// 255577722 - 70298736 = 185278986
//
// Currently the Terraswap pair has:
// 173.3 ANC
// 1047528002 uusd
// 425812853 total shares
//
// Burning this many LP tokens should get us these much ANC and UST:
// 173300000 * 70298736 / 425812853 = 28610622 uANC (28.610622 ANC)
// 1047528002 * 70298736 / 425812853 = 172939576 uusd
//
// The remaining balance at Terraswap pair should be:
// 173300000 - 28610622 = 144689378 uANC
// 1047528002 - 172939576 = 874588426 uusd
// 425812853 - 70298736 = 355514117 uLP
//
// The strategy will actually receive
// deduct_tax(172939576) = 172766809 uusd
//
// User 1 has 320099901 uusd debt, greater than 172766809, so the proceeding will all be
// used to pay back debt
//
// After tax, the deliverable amount is
// deduct_tax(172766809) = 172594214
//
// The strategy's total debt should be
// 320099901 - 172594214 = 147505687 uusd
//
// The user's debt unit should be reduced by 172594214000000, to
// 320099901000000 - 172594214000000 = 147505687000000
//
// State:
// totalAssetUnits = 254110517355205 - 69895170000000 = 184215347355205
// totalDebtUnits = 320099901000000 - 172594214000000 = 147505687000000
//
//------------------------------- DEBT RATIO CALCULATION ---------------------------------
//
// Value per LP token:
// 2 * 874588426 / 355514117 = 4.920133317800148003 uusd/uLP
//
// Strategy:
// Asset value = 185278986 * 4.920133317800148003 = 911597314 uusd
// Debt value = 147505687 uusd
// Debt ratio = 147505687 / 911597314 = 0.161810137803894406
//
// User 1:
// LP tokens staked = 185278986 * 100000000000000 / 184215347355205 = 100577388 uLP
// Asset value = 100577388 * 4.920133317800148003 = 494854158 uusd
// Debt value = 147505687 uusd
// Debt ratio = 147505687 / 494854158 = 0.298079110007195291
//
// User 2:
// LP tokens staked = 185278986 * 84215347355205 / 184215347355205 = 84701597 uLP
// Asset value = 84701597 * 4.920133317800148003 = 416743150 uusd
//----------------------------------------------------------------------------------------

async function testReducePosition1() {
  process.stdout.write("Should reduce position... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, field, {
      reduce_position: {
        bond_units: "69895170000000",
        remove: true,
        repay: true,
      },
    }),
  ]);

  // State
  await verifyState({
    total_bond_units: "184215347355205",
    total_debt_units: "147505687000000",
  });
  await verifyHealth(null, {
    bond_value: "911597314",
    debt_value: "147505687",
    ltv: "0.161810137803894406",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "100000000000000",
    debt_units: "147505687000000",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user1, {
    bond_value: "494854158",
    debt_value: "147505687",
    ltv: "0.298079110007195291",
  });
  await verifySnapshot(
    user1,
    {
      is_active: true,
      bond_units: "100000000000000",
      debt_units: "147505687000000",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "494854158",
      debt_value: "147505687",
      ltv: "0.298079110007195291",
    }
  );

  // User 2
  await verifyPosition(user2, {
    is_active: true,
    bond_units: "84215347355205",
    debt_units: "0",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user2, {
    bond_value: "416743150",
    debt_value: "0",
    ltv: "0",
  });
  await verifySnapshot(
    user2,
    {
      is_active: true,
      bond_units: "84215347355205",
      debt_units: "0",
      unlocked_assets: unlockedAssets,
    },
    {
      bond_value: "416743144",
      debt_value: "0",
      ltv: "0",
    }
  );

  // Bond & debt amounts
  await verifyBond("185278986");
  await verifyDebt("147505687");

  // User 1 should have received 28610622 uANC
  const user1AncBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    anchorToken
  );
  expect(user1AncBalance).to.equal("28610622");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Liquidation, Pt. 1
//
//--------------------------------- Part 1. Dump tokens ----------------------------------
//
// First, we dump a large amount of ANC in the market. This should crash ANC price and
// cause user 1's debt ratio to be above the threshold
//
// User 2 should be fine, however, since he does not take on any debt
//
// Prior to the swap, the pool has:
// 144689378 uANC
// 874588426 uusd
// 355514117 uLP
//
// kValueBefore = 144689378 * 874588426 = 126543655363939028
//
// returnUstAmount = 874588426 - 126543655363939028 / (144689378 + 500000000)
// = 678302183 uusd
//
// fee = 678302183 * 0.003 = 2034906 uusd
//
// After the swap, the pool should have
// 144689378 + 500000000 = 644689378 uANC
// 874588426 - 678302183 + 2034906 = 198321149 uusd
// 355514117 uLP (unchanged)
//
// ANC price
// before swap: 874588426 / 144689378 = 6.04 UST
// after swap: 198321149 / 644689378 = 0.3076 UST
//
// Value per LP token:
// 2 * 198321149 / 355514117 = 1.115686491853149111 uusd/uLP
//
// Strategy:
// Asset value = 185278986 * 1.115686491853149111 = 206713261 uusd
// Debt value = 147505687 uusd (unchanged)
// Debt ratio = 147505687 / 206713261 = 0.713576314777405596
//
// User 1:
// LP tokens staked = 100577388 uLP (unchanged)
// Asset value = 100577388 * 1.115686491853149111 = 112212833 uusd
// Debt value = 147505687 uusd (unchanged)
// Debt ratio = 147505687 / 112212833 = 1.314517092710777563
//
// User 1 is insolvent (more debt than collateral). In practice, if liquidators are
// efficient, this shouldn't happen.
//
// User 2:
// LP tokens staked = 84701597 uLP (unchanged)
// Asset value = 84701597 * 1.115686491853149111 = 94500427 uusd
//
//--------------------------------- Part 2. Liquidation ----------------------------------
//
// At this time, the strategy has:
// 185278986 uLP staked
// 147505687 uusd debt
//
// Terraswap pool has:
// 644689378 uANC
// 198321149 uusd
// 355514117 uLP
//
//---------------------------------- POSITION CLOSURE ----------------------------------
//
// First, user's position needs to be closed. The amount of LP tokens to be unstaked:
// 185278986 * 100000000000000 / 184215347355205
// = 100577388 uLP
//
// Which should return:
// 644689378 * 100577388 / 355514117 = 182387057 uANC
// 198321149 * 100577388 / 355514117 = 56106416 uusd
//
// The amount the strategy will actually receive is
// deductTax(56106416) = 56050365
//
// When repaying debt, the amount Mars will receive is
// deductTax(56050365) = 55994370
// 55994370000000 debt units should be reduced.
//
// Terraswap state after closure:
// 644689378 - 182387057 = 462302321 uANC
// 198321149 - 56106416 = 142214733 uusd
// 355514117 - 100577388 = 254936729 uLP
//
// Value per LP token:
// 2 * 142214733 / 254936729 = 1.115686496471836351
//
// Strategy state after closure:
// 185278986 - 100577388 = 84701598 uLP staked
// 147505687 - 55994370 = 91511317 uusd debt
//
// total_bond_value = 84701598 * 1.115686496471836351 = 94500429 uusd
// total_bond_units = 184215347355205 - 100000000000000 = 84215347355205
// total_debt_value = 91511317 uusd
// total_debt_units = 147505687000000 - 55994370000000 = 91511317000000
//
// The user's position after closure:
// unbonded_anc_amount = 182387057
// debt_units = 147505687000000 - 55994370000000 = 91511317000000
// debt_value = 91511317
//
//---------------------------------- CLAIM COLLATERAL ----------------------------------
//
// Liquidator sends 50000000 uusd. The amount deliverable to Mars:
// deductTax(50000000) = 49950049
//
// 49950049000000 debt units should be reduced
//
// ANC to be released:
// 182387057 * 49950049 / 91511317 = 99553178 uANC
//
// Strategy state after claim collateral:
// 84701598 uLP staked (unchanged)
// 91511317 - 49950049 = 41561268 uusd debt
//
// total_bond_value = 94500429 uusd (unchanged)
// total_bond_units = 84215347355205 (unchanged)
// total_debt_value = 41561268 uusd
// total_debt_units = 91511317000000 - 49950049000000 = 41561268000000
// ltv = 41561268 / 94500429 = 0.439799781226389988
//
// User 1 position:
// unbonded_anc_amount = 182387057 - 99553178 = 82833879
// debt_units = 91511317000000 - 49950049000000 = 41561268000000
// debt_value = 41561268
//
// User 2 position:
// bond_value = 94500429 (same as strategy)
// bond_units = 84215347355205 (unchanged)
//----------------------------------------------------------------------------------------

async function testLiquidation1() {
  process.stdout.write("Should partially liquidate user 1... ");

  // Part 1. Dump tokens
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      send: {
        amount: "500000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  // State
  await verifyState({
    total_bond_units: "184215347355205",
    total_debt_units: "147505687000000",
  });
  await verifyHealth(null, {
    bond_value: "206713261",
    debt_value: "147505687",
    ltv: "0.713576314777405596",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: true,
    bond_units: "100000000000000",
    debt_units: "147505687000000",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user1, {
    bond_value: "112212833",
    debt_value: "147505687",
    ltv: "1.314517092710777563",
  });

  // User 2
  await verifyPosition(user2, {
    is_active: true,
    bond_units: "84215347355205",
    debt_units: "0",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user2, {
    bond_value: "94500427",
    debt_value: "0",
    ltv: "0",
  });

  // Bond & debt amounts
  await verifyBond("185278986");
  await verifyDebt("147505687");

  // Uncomment this to abort after Part 1
  // return;

  // Part 2. Liquidation
  await sendTransaction(terra, liquidator1, [
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
            amount: "50000000",
          },
        },
      },
      {
        uusd: "50000000",
      }
    ),
  ]);

  // State
  await verifyState({
    total_bond_units: "84215347355205",
    total_debt_units: "41561268000000",
  });
  await verifyHealth(null, {
    bond_value: "94500429",
    debt_value: "41561268",
    ltv: "0.439799781226389988",
  });

  // User 1
  await verifyPosition(user1, {
    is_active: false,
    bond_units: "0",
    debt_units: "41561268000000", // still have some debt left
    unlocked_assets: [
      {
        info: {
          token: {
            contract_addr: anchorToken,
          },
        },
        amount: "82833879", // this is the amount of ANC not yet liquidated
      },
      {
        info: {
          native_token: {
            denom: "uusd",
          },
        },
        amount: "0",
      },
      {
        info: {
          token: {
            contract_addr: terraswapLpToken,
          },
        },
        amount: "0",
      },
    ],
  });
  await verifyHealth(user1, {
    bond_value: "0",
    debt_value: "41561268", // still have some debt left
    ltv: null, // Option<T>::None is serialized as null
  });

  // User 2
  await verifyPosition(user2, {
    is_active: true,
    bond_units: "84215347355205",
    debt_units: "0",
    unlocked_assets: unlockedAssets,
  });
  await verifyHealth(user2, {
    bond_value: "94500429",
    debt_value: "0",
    ltv: "0",
  });

  // Bond & debt amounts
  await verifyBond("84701598");
  await verifyDebt("41561268");

  // Liquidator should have receive correct amount of ANC token
  const liquidatorAncBalance = await queryTokenBalance(
    terra,
    liquidator1.key.accAddress,
    anchorToken
  );
  expect(liquidatorAncBalance).to.equal("99553178");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Liquidation, Pt. 2
//
// User 1 has 41561268 uusd debt remaining. To fully pay down these, liquidator needs:
// deduct_tax(x) = 41561268 => x = 41602830
//----------------------------------------------------------------------------------------

async function testLiquidation2() {
  process.stdout.write("Should completely liquidate user 1... ");

  await sendTransaction(terra, liquidator2, [
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
            amount: "41602830",
          },
        },
      },
      {
        uusd: "41602830",
      }
    ),
  ]);

  // State
  await verifyState({
    total_bond_units: "84215347355205",
    total_debt_units: "0",
  });
  await verifyHealth(null, {
    bond_value: "94500429",
    debt_value: "0",
    ltv: "0",
  });

  // Debt amount: should be zero
  await verifyDebt("0");

  // User 1 is fully liquidated, so their position and snapshot should have been purged
  // from storage. Querying it should fail with statue code 500
  await expect(verifyPosition(user1, {})).to.be.rejectedWith("status code 500");
  await expect(verifySnapshot(user1, {}, {})).to.be.rejectedWith("status code 500");

  // Liquidator should have received all of user 1's unstaked ANC, which is 82833879
  const liquidatorAncBalance = await queryTokenBalance(
    terra,
    liquidator2.key.accAddress,
    anchorToken
  );
  expect(liquidatorAncBalance).to.equal("82833879");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Reduce Position, Pt. 2
//
// User 2 closes position completely and withdraw all assets by not providing optional
// `bond_units` argument
//
// Prior to withdrawal, strategy has 84701598 uLP staked, which all belong to user 2
//
// Terraswap has:
// 462302321 uANC
// 142214733 uusd
// 254936729 uLP
//
// Burning 84701598 uLP should get
// 462302321 * 84701598 / 254936729 = 153597896 uANC
// 142214733 * 84701598 / 254936729 = 47250214 uusd
//
// The actual amount of uusd deliverable to the user is
// deductTax(deductTax(47250214)) = 47155854
//----------------------------------------------------------------------------------------

async function testReducePosition2() {
  process.stdout.write("Should close position... ");

  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, field, {
      reduce_position: {
        bond_units: null, // burn all bond units,
        remove: false, // do not remove liquidity; return share tokens directly
        repay: true, // should skip as there is no UST to repay
      },
    }),
  ]);

  // State
  await verifyState({
    total_bond_units: "0",
    total_debt_units: "0",
  });
  await verifyHealth(null, {
    bond_value: "0",
    debt_value: "0",
    ltv: null,
  });

  // User's position as well as the snaphot should have been deleted
  await expect(verifyPosition(user2, {})).to.be.rejectedWith("status code 500");
  await expect(verifySnapshot(user2, {}, {})).to.be.rejectedWith("status code 500");

  // User should have receive correct amount of UST and ANC
  const userShareTokenBalance = await queryTokenBalance(
    terra,
    user2.key.accAddress,
    terraswapLpToken
  );
  expect(userShareTokenBalance).to.equal("84701598");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Update Config
//----------------------------------------------------------------------------------------

async function testUpdateConfig() {
  process.stdout.write("Should update config... ");

  const newConfig = config as { fee_rate: string };
  newConfig.fee_rate = "1"; // change performance fee rate to 100%

  const executeMsg = {
    update_config: {
      new_config: newConfig,
    },
  };

  // Try updating config with a non-owner user; should fail
  await expect(
    sendTransaction(terra, user1, [
      new MsgExecuteContract(user1.key.accAddress, field, executeMsg),
    ])
  ).to.be.rejectedWith("unauthorized");

  // Try updating with owner account; should succeed
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, field, executeMsg),
  ]);

  await verifyConfig(newConfig);

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test: Migration
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
      new MsgMigrateContract(user1.key.accAddress, field, newCodeId, {}),
    ])
  ).to.be.rejectedWith("unauthorized");

  // Try migrate with owner; should throw "unimplemented" error
  // Note: the owner at WASM level at the one at contract level are separate and may be
  // different; however, in this test, they are both set to `deployer`
  await expect(
    sendTransaction(terra, deployer, [
      new MsgMigrateContract(deployer.key.accAddress, field, newCodeId, {}),
    ])
  ).to.be.rejectedWith("unimplemented");

  // Check code ID
  // const codeId = (await terra.wasm.contractInfo(strategy)).code_id;
  // expect(codeId).to.equal(newCodeId);

  console.log(chalk.green("Passed!"));
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

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Strategy: ANC-UST LP"));

  await testConfig();
  await testOpenPosition1();
  await testHarvest();
  await testOpenPosition2();
  await testPayDebt();
  await testReducePosition1();
  await testLiquidation1();
  await testLiquidation2();
  await testReducePosition2();
  await testUpdateConfig();
  await testMigrate();

  console.log("");
})();

//----------------------------------------------------------------------------------------
// Helper Functions
//----------------------------------------------------------------------------------------

async function verifyConfig(expectedResponse: object) {
  const response = await terra.wasm.contractQuery(field, {
    config: {},
  });

  // console.log("\nexpected:", expectedResponse);
  // console.log("actual:", response);

  expect(response).to.deep.equal(expectedResponse);
}

async function verifyState(expectedResponse: object) {
  const response = await terra.wasm.contractQuery(field, {
    state: {},
  });

  // console.log("\nexpected:", expectedResponse);
  // console.log("actual:", response);

  expect(response).to.deep.equal(expectedResponse);
}

async function verifyPosition(user: Wallet, expectedResponse: object) {
  const response = await terra.wasm.contractQuery(field, {
    position: {
      user: user.key.accAddress,
    },
  });

  // console.log("\nexpected:", expectedResponse);
  // console.log("actual:", response);

  expect(response).to.deep.equal(expectedResponse);
}

async function verifyHealth(user: Wallet | null, expectedResponse: object) {
  const response = await terra.wasm.contractQuery(field, {
    health: {
      user: user ? user.key.accAddress : null,
    },
  });

  // console.log("\nexpected:", expectedResponse);
  // console.log("actual:", response);

  expect(response).to.deep.equal(expectedResponse);
}

async function verifySnapshot(
  user: Wallet,
  expectedPosition: object,
  expectedHealth: object
) {
  const response = (await terra.wasm.contractQuery(field, {
    snapshot: {
      user: user.key.accAddress,
    },
  })) as {
    health: object;
    position: object;
  };

  // console.log("\nexpected position:", expectedPosition);
  // console.log("actual position:", response.position);

  // console.log("\nexpected health:", expectedHealth);
  // console.log("actual health:", response.health);

  expect(response.position).to.deep.equal(expectedPosition);
  expect(response.health).to.deep.equal(expectedHealth);
}

async function verifyDebt(expectedAmount: string) {
  const response = (await terra.wasm.contractQuery(redBank, {
    debt: {
      address: field,
    },
  })) as {
    debts: { denom: string; amount: string }[];
  };

  const debt = response.debts.find((debt) => {
    return debt.denom == "uusd";
  })?.amount;

  // console.log("\nexpected:", expectedAmount);
  // console.log("actual:", debt);

  expect(debt).to.equal(expectedAmount);
}

async function verifyBond(expectedAmount: string) {
  const response = (await terra.wasm.contractQuery(anchorStaking, {
    staker_info: {
      staker: field,
      block_height: null,
    },
  })) as { bond_amount: string };

  // console.log("expected:", expectedAmount);
  // console.log("actual:", response.bond_amount);

  expect(response.bond_amount).to.equal(expectedAmount);
}
