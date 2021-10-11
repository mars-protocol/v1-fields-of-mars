import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployRedBank } from "./fixture";
import { GAS_AMOUNT, deductTax, queryNativeBalance, sendTransaction } from "./helpers";
import { Contract } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let redBank: Contract;

interface UserAssetDebtResponse {
  denom: string;
  amount: string;
}

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  redBank = await deployRedBank(terra, deployer);

  process.stdout.write("Fund contract with LUNA and UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(
      deployer.key.accAddress,
      redBank.address,
      { uluna: 100000000, uusd: 100000000 } // fund contract with 100 LUNA + 100 UST
    ),
  ]);

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Borrow
//--------------------------------------------------------------------------------------------------

async function testBorrow() {
  process.stdout.write("1. Borrow Luna... ");

  const userLunaBalanceBefore = await queryNativeBalance(terra, user.key.accAddress, "uluna");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, redBank.address, {
      borrow: {
        asset: {
          native: {
            denom: "uluna",
          },
        },
        amount: "42000000", // borrow 42 LUNA
      },
    }),
  ]);

  const userLunaBalanceAfter = await queryNativeBalance(terra, user.key.accAddress, "uluna");

  // Note: transfer of LUNA is not subject to tax
  expect(parseInt(userLunaBalanceAfter) - parseInt(userLunaBalanceBefore)).to.equal(
    42000000 - GAS_AMOUNT
  );

  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(redBank.address, {
    user_asset_debt: {
      user_address: user.key.accAddress,
      asset: {
        native: {
          denom: "uluna",
        },
      },
    },
  });

  expect(response.amount).to.equal("42000000");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Repay
//--------------------------------------------------------------------------------------------------

async function testRepay() {
  process.stdout.write("2. Repay LUNA... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(
      user.key.accAddress,
      redBank.address,
      {
        repay_native: {
          denom: "uluna",
        },
      },
      { uluna: 12345678 }
    ),
  ]);

  // 42000000 - 12345678 = 29654322 uluna
  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(redBank.address, {
    user_asset_debt: {
      user_address: user.key.accAddress,
      asset: {
        native: {
          denom: "uluna",
        },
      },
    },
  });

  expect(response.amount).to.equal("29654322");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 3. Set Debt
//--------------------------------------------------------------------------------------------------

async function testSetUserDebt() {
  process.stdout.write("3. [mock] Set user debt... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, redBank.address, {
      set_user_debt: {
        user_address: user.key.accAddress,
        denom: "uluna",
        amount: "69420",
      },
    }),
  ]);

  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(redBank.address, {
    user_asset_debt: {
      user_address: user.key.accAddress,
      asset: {
        native: {
          denom: "uluna",
        },
      },
    },
  });

  expect(response.amount).to.equal("69420");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user.key.accAddress)} as user`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Mock Red Bank"));

  await testBorrow();
  await testRepay();
  await testSetUserDebt();

  console.log("");
})();
