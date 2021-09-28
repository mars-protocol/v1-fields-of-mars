import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployCw20Token, deployAstroport } from "./fixture";
import { queryNativeBalance, queryCw20Balance, sendTransaction, toEncodedBinary } from "./helpers";
import { Contract, Protocols } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;
const user2 = terra.wallets.test3;

let cw20Token: Contract;
let astroport: Protocols.Astroport;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  cw20Token = await deployCw20Token(terra, deployer);
  astroport = await deployAstroport(terra, deployer, cw20Token);

  process.stdout.write("Fund user1 with tokens... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, cw20Token.address, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "10000000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user2 with tokens... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, cw20Token.address, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "10000000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Provide Initial Liquidity
//
// User1 provides 1_000_000_000 uusd + 100_000_000 uMIR (price: 1 MIR = 10 UST)
// User1 should receive sqrt(1_000_000_000 * 100_000_000) = 316227766 uLP
//
// Result
// ---
// pool uusd  1000000000
// pool uMIR  100000000
// pool uLP   316227766
//--------------------------------------------------------------------------------------------------

async function testProvideInitialLiquidity() {
  process.stdout.write("1. Provide initial liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, cw20Token.address, {
      increase_allowance: {
        amount: "100000000",
        spender: astroport.pair.address,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      astroport.pair.address,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "1000000000",
            },
            {
              info: {
                token: {
                  contract_addr: cw20Token.address,
                },
              },
              amount: "100000000",
            },
          ],
        },
      },
      {
        uusd: "1000000000",
      }
    ),
  ]);

  const poolUusd = await queryNativeBalance(terra, astroport.pair.address);
  expect(poolUusd).to.equal("1000000000");

  const poolUMir = await queryCw20Balance(terra, astroport.pair.address, cw20Token.address);
  expect(poolUMir).to.equal("100000000");

  const poolULp = await queryCw20Balance(terra, user1.key.accAddress, astroport.shareToken.address);
  expect(poolULp).to.equal("316227766");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Provide Further Liquidity
//
// User1 provides another 690000000 uusd + 69000000 uMIR
//
// The amount of LP token the user should receive is:
// min(ustDeposit * totalShare / ustBalance, mirDeposit * totalShare / mirBalance)
// = min(690e6 * 316227766 / 1000e6, 69e6 * 316227766 / 100e6)
// = min(218197158, 218197158)
// = 218197158
//
// Result
// ---
// pool uusd  1000000000 + 690000000 = 1690000000
// pool uMIR  100000000 + 69000000 = 169000000
// pool uLP   316227766 + 218197158 = 534424924
//--------------------------------------------------------------------------------------------------

async function testProvideFurtherLiquidity() {
  process.stdout.write("2. Provide further liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, cw20Token.address, {
      increase_allowance: {
        amount: "69000000",
        spender: astroport.pair.address,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      astroport.pair.address,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "690000000",
            },
            {
              info: {
                token: {
                  contract_addr: cw20Token.address,
                },
              },
              amount: "69000000",
            },
          ],
        },
      },
      {
        uusd: "690000000",
      }
    ),
  ]);

  const poolUusd = await queryNativeBalance(terra, astroport.pair.address);
  expect(poolUusd).to.equal("1690000000");

  const poolUMir = await queryCw20Balance(terra, astroport.pair.address, cw20Token.address);
  expect(poolUMir).to.equal("169000000");

  const poolULp = await queryCw20Balance(terra, user1.key.accAddress, astroport.shareToken.address);
  expect(poolULp).to.equal("534424924");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 3. Swap
//
// User2 sells 100 MIR for UST
//
// kValueBefore = poolUstBalance * poolMirBalance
// = 1690000000 * 169000000 = 285610000000000000;
// returnAmount = poolUstBalance - kValueBefore / (poolMirBalance + sendMirAmount)
// = 1690000000 - 285610000000000000 / (169000000 + 100000000)
// = 628252789
// fee = returnAmount * feeRate
// = 628252789 * 0.003
// = 1884758
// returnAmountAfterFee = returnUstAmount - fee
// = 628252789 - 1884758
// = 626368031
// returnAmountAfterFeeAndTax = deductTax(626368031) = 625742288
// transaction cost for pool = addTax(625742288) = 626368030
//
// Result
// ---
// pool uusd  1690000000 - 626368030 = 1063631970
// pool uMIR  169000000 + 100000000 = 269000000
// pool uLP   534424924
//--------------------------------------------------------------------------------------------------

async function testSwap() {
  process.stdout.write("3. Swap... ");
  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, cw20Token.address, {
      send: {
        amount: "100000000",
        contract: astroport.pair.address,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  const poolUusd = await queryNativeBalance(terra, astroport.pair.address);
  expect(poolUusd).to.equal("1063631970");

  const poolUMir = await queryCw20Balance(terra, astroport.pair.address, cw20Token.address);
  expect(poolUMir).to.equal("269000000");

  const poolULp = await queryCw20Balance(terra, user1.key.accAddress, astroport.shareToken.address);
  expect(poolULp).to.equal("534424924");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 4. Remove Liquidity
//
// User1 burns 420 LP tokens
//
// uusd to be released = 1063631970 * 420000000 / 534424924 = 835899314
// uMIR to be released = 269000000 * 420000000 / 534424924 = 211404810
// transaction cost for sending UST: addTax(deductTax(835899314)) = 835899313
//
// pool uusd  1063631970 - 835899313 = 227732657
// pool uMIR  269000000 - 211404810 = 57595190
// pool uLP   534424924 - 420000000 = 114424924
//--------------------------------------------------------------------------------------------------

async function testRemoveLiquidity() {
  process.stdout.write("4. Remove liquidity... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, astroport.shareToken.address, {
      send: {
        amount: "420000000",
        contract: astroport.pair.address,
        msg: toEncodedBinary({
          withdraw_liquidity: {},
        }),
      },
    }),
  ]);

  const poolUusd = await queryNativeBalance(terra, astroport.pair.address);
  expect(poolUusd).to.equal("227732657");

  const poolUMir = await queryCw20Balance(terra, astroport.pair.address, cw20Token.address);
  expect(poolUMir).to.equal("57595190");

  const poolULp = await queryCw20Balance(terra, user1.key.accAddress, astroport.shareToken.address);
  expect(poolULp).to.equal("114424924");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Astroport"));

  await testProvideInitialLiquidity();
  await testProvideFurtherLiquidity();
  await testSwap();
  await testRemoveLiquidity();

  console.log("");
})();
