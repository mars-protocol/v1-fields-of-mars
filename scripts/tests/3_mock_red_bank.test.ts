import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployRedBank } from "./fixture";
import { queryNativeBalance } from "../helpers/queries";
import { sendTransaction } from "../helpers/tx";
import { UserAssetDebtResponse } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let bank: string;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  ({ bank } = await deployRedBank(deployer));

  process.stdout.write("Fund contract with LUNA and UST...");

  await sendTransaction(deployer, [
    new MsgSend(deployer.key.accAddress, bank, { uluna: 100000000, uusd: 100000000 }),
  ]);

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Borrow
//--------------------------------------------------------------------------------------------------

async function testBorrow() {
  process.stdout.write("1. Borrow Luna... ");

  const userLunaBalanceBefore = await queryNativeBalance(terra, user.key.accAddress, "uluna");

  await sendTransaction(user, [
    new MsgExecuteContract(user.key.accAddress, bank, {
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

  expect(parseInt(userLunaBalanceAfter) - parseInt(userLunaBalanceBefore)).to.equal(42000000);

  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(bank, {
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

  await sendTransaction(user, [
    new MsgExecuteContract(
      user.key.accAddress,
      bank,
      {
        repay_native: {
          denom: "uluna",
        },
      },
      { uluna: 12345678 }
    ),
  ]);

  // 42000000 - 12345678 = 29654322 uluna
  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(bank, {
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

  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, bank, {
      set_user_debt: {
        user_address: user.key.accAddress,
        denom: "uluna",
        amount: "69420",
      },
    }),
  ]);

  const response: UserAssetDebtResponse = await terra.wasm.contractQuery(bank, {
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
  console.log(chalk.yellow("\nInfo"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user.key.accAddress)} as user`);

  console.log(chalk.yellow("\nSetup"));

  await setupTest();

  console.log(chalk.yellow("\nTests"));

  await testBorrow();
  await testRepay();
  await testSetUserDebt();

  console.log("");
})();
