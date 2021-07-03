import chalk from "chalk";
import { LocalTerra, MsgExecuteContract, MsgSend } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployMockMars } from "./fixture";
import {
  GAS_AMOUNT,
  deductTax,
  queryNativeTokenBalance,
  sendTransaction,
} from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let mars: string;

async function setupTest() {
  mars = await deployMockMars(terra, deployer);

  process.stdout.write("Fund contract with UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(deployer.key.accAddress, mars, { uusd: 100000000 }),
  ]);

  console.log(chalk.green("Done!"));
}

async function testBorrow() {
  process.stdout.write("Should handle borrowing UST... ");

  const userUstBalanceBefore = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uusd"
  );

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mars, {
      borrow: {
        asset: {
          native: {
            denom: "uusd",
          },
        },
        amount: "69000000",
      },
    }),
  ]);

  const userUstBalanceAfter = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uusd"
  );

  // User should have received correct amount of UST
  // Note: 0.1% tax is charged on all UST transfers. If we borrow 69 UST, should expect to
  // receive 69 * 99.9% = 68.931 UST (68931000 uusd)
  expect(parseInt(userUstBalanceAfter) - parseInt(userUstBalanceBefore)).to.equal(
    deductTax(69000000) - GAS_AMOUNT
  );

  // Contract should return correct amount of debt
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uusd",
        amount: "69000000",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testRepay() {
  process.stdout.write("Should handle repaying UST... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(
      user.key.accAddress,
      mars,
      {
        repay_native: {
          denom: "uusd",
        },
      },
      { uusd: 12345678 }
    ),
  ]);

  // User pays 12.345678 UST, should have 69 - 12.345678 = 56.654322 UST debt remaining
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uusd",
        amount: "56654322",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

(async () => {
  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Mock Mars"));

  await testBorrow();
  await testRepay();

  console.log("");
})();
