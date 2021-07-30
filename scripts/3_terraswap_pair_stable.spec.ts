import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployTerraswapPair, deployTerraswapToken } from "./fixture";
import {
  queryNativeTokenBalance,
  queryTokenBalance,
  sendTransaction,
  toEncodedBinary,
} from "./helpers";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;
const user2 = terra.wallets.test3;

let bLunaToken: string;
let terraswapPair: string;
let terraswapLpToken: string;

//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function setupTest() {
  let { cw20CodeId, cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock Bonded Luna",
    "bLUNA"
  );
  bLunaToken = cw20Token;

  // Note: assets[0] = bLuna, assets[1] = Luna
  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(
    terra,
    deployer,
    {
      asset_infos: [
        {
          token: {
            contract_addr: bLunaToken,
          },
        },
        {
          native_token: {
            denom: "uluna",
          },
        },
      ],
      token_code_id: cw20CodeId,
    },
    true // deploy stable pair
  ));

  process.stdout.write("Fund user1 with bLUNA... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bLunaToken, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "700000000000", // 700,000 bLuna
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user2 with bLUNA... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bLunaToken, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "100000000000", // 100,000 bLUNA
      },
    }),
  ]);

  console.log(chalk.green("Done!"));
}

//----------------------------------------------------------------------------------------
// Test 1. Provide Initial Liquidity
//
// User1 provides the following amount:
// 722_090_275_787 uluna
// 698_810_752_552 ubluna
// These are actual amounts from a snapshot of mainnet bLUNA-LUNA pair
//
// User1 should receive sqrt(698810752552 * 722090275787) = 710355156969 uLP
//
// Result
// ---
// pool ubluna  698810752552
// pool uluna   722090275787
// pool uLP     710355156969
//----------------------------------------------------------------------------------------

async function testProvideInitialLiquidity() {
  process.stdout.write("Should handle providing initial liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, bLunaToken, {
      increase_allowance: {
        amount: "698810752552",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      terraswapPair,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                token: {
                  contract_addr: bLunaToken,
                },
              },
              amount: "698810752552",
            },
            {
              info: {
                native_token: {
                  denom: "uluna",
                },
              },
              amount: "722090275787",
            },
          ],
        },
      },
      {
        uluna: "722090275787",
      }
    ),
  ]);

  const blunaBalance = await queryTokenBalance(terra, terraswapPair, bLunaToken);
  expect(blunaBalance).to.equal("698810752552");

  const lunaBalance = await queryNativeTokenBalance(terra, terraswapPair, "uluna");
  expect(lunaBalance).to.equal("722090275787");

  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("710355156969");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 2. Provide Further Liquidity
//
// User1 provides another 69000000 ubluna + 69000000 uluna
//
// The amount of LP token the user should receive is:
// min(blunaDeposit * totalShare / blunaBalance, lunaDeposit * totalShare / lunaBalance)
// = min(69000000 * 710355156969 / 698810752552, 69000000 * 710355156969 / 722090275787)
// = min(70139885, 67878639)
// = 67878639
//
// Result
// ---
// pool ubluna  698810752552 + 69000000 = 698879752552
// pool uluna   722090275787 + 69000000 = 722159275787
// pool uLP     710355156969 + 67878639 = 710423035608
//----------------------------------------------------------------------------------------

async function testProvideFurtherLiquidity() {
  process.stdout.write("Should handle providing further liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, bLunaToken, {
      increase_allowance: {
        amount: "69000000",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      terraswapPair,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                token: {
                  contract_addr: bLunaToken,
                },
              },
              amount: "69000000",
            },
            {
              info: {
                native_token: {
                  denom: "uluna",
                },
              },
              amount: "69000000",
            },
          ],
        },
      },
      {
        uluna: "69000000",
      }
    ),
  ]);

  const blunaBalance = await queryTokenBalance(terra, terraswapPair, bLunaToken);
  expect(blunaBalance).to.equal("698879752552");

  const lunaBalance = await queryNativeTokenBalance(terra, terraswapPair, "uluna");
  expect(lunaBalance).to.equal("722159275787");

  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("710423035608");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 3. Swap
//
// User2 sells 100,000 bLUNA for LUNA
// offer_amount = 100_000_000_000
// amp = 100
// n_coins = 2
// leverage = amp * n_coins = 2
//
// For calculations, refer to `./math.ts`
// d = computeD(leverage, poolBLuna, poolLuna) = 1421037139888
// poolBLunaAfter = poolBLuna + offerAmount = 798879752552
// poolLunaAfter = computeNewBalanceOut(leverage, poolBLunaAfter, d) = 622267721247
// swapAmount = poolLuna - poolLunaAfter = 99891554540
// commissionAmount = swapAmount * 0.003 = 299674663
// returnAmount = swapAmount - commissionAmount = 99591879877
// returnAmountAfterTax = deductTax(returnAmount) = 99591879877 (0 tax for LUNA transfers)
//
// pool bluna  698879752552 + 100000000000 = 798879752552
// pool luna   722159275787 - 99591879877 = 622567395910
//
// Result
// ---
// pool ubluna  798879752552
// pool uluna   622567395910
// pool uLP     710423035608
//----------------------------------------------------------------------------------------

async function testSwap() {
  process.stdout.write("Should handle swaps... ");

  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, bLunaToken, {
      send: {
        amount: "100000000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  const blunaBalance = await queryTokenBalance(terra, terraswapPair, bLunaToken);
  expect(blunaBalance).to.equal("798879752552");

  const lunaBalance = await queryNativeTokenBalance(terra, terraswapPair, "uluna");
  expect(lunaBalance).to.equal("622567395910");

  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("710423035608");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 4. Remove Liquidity
//
// User1 burns 420 LP tokens
//
// bLuna to be released = 798879752552 * 420000000 / 710423035608 = 472295349
// Luna to be released = 622567395910 * 420000000 / 710423035608 = 368060005
//
// pool ubluna  798879752552 - 472295349 = 798407457203
// pool uluna   622567395910 - 368060005 = 622199335905
// pool uLP     710423035608 - 420000000 = 710003035608
//
// Result
// ---
// pool ubluna  798407457203
// pool uluna   622199335905
// pool uLP     710003035608
//----------------------------------------------------------------------------------------

async function testRemoveLiquidity() {
  process.stdout.write("Should handle removal of liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, terraswapLpToken, {
      send: {
        amount: "420000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          withdraw_liquidity: {},
        }),
      },
    }),
  ]);

  const blunaBalance = await queryTokenBalance(terra, terraswapPair, bLunaToken);
  expect(blunaBalance).to.equal("798407457203");

  const lunaBalance = await queryNativeTokenBalance(terra, terraswapPair, "uluna");
  expect(lunaBalance).to.equal("622199335905");

  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("710003035608");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: TerraSwap Pair"));

  await testProvideInitialLiquidity();
  await testProvideFurtherLiquidity();
  await testSwap();
  await testRemoveLiquidity();

  console.log("");
})();
