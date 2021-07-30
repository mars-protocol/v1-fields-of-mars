import chalk from "chalk";
import {
  LocalTerra,
  MsgExecuteContract,
  MsgMigrateContract,
  MsgSend,
} from "@terra-money/terra.js";
import { expect } from "chai";
import { deployMockMars } from "./fixture";
import {
  GAS_AMOUNT,
  deductTax,
  queryNativeTokenBalance,
  sendTransaction,
} from "./helpers";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let redBank: string;

//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function setupTest() {
  redBank = await deployMockMars(terra, deployer);

  process.stdout.write("Fund contract with LUNA and UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(
      deployer.key.accAddress,
      redBank,
      { uluna: 100000000, uusd: 100000000 } // fund contract with 100 LUNA + 100 UST
    ),
  ]);

  console.log(chalk.green("Done!"));
}

//----------------------------------------------------------------------------------------
// Test 1. Borrow, Pt. 1
//----------------------------------------------------------------------------------------

async function testBorrow1() {
  process.stdout.write("Should handle borrowing LUNA... ");

  const userLunaBalanceBefore = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uluna"
  );

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, redBank, {
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

  const userLunaBalanceAfter = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uluna"
  );

  // Note: transfer of LUNA is not subject to tax
  expect(parseInt(userLunaBalanceAfter) - parseInt(userLunaBalanceBefore)).to.equal(
    42000000 - GAS_AMOUNT
  );

  const debtResponse = await terra.wasm.contractQuery(redBank, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "42000000",
      },
      {
        denom: "uusd",
        amount: "0",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 2. Borrow, Pt. 2
//----------------------------------------------------------------------------------------

async function testBorrow2() {
  process.stdout.write("Should handle borrowing UST... ");

  const userUstBalanceBefore = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uusd"
  );

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, redBank, {
      borrow: {
        asset: {
          native: {
            denom: "uusd",
          },
        },
        amount: "69000000", // borrow 69 UST
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

  // With mockInterestRate = 1.1, debt amount should be 69000000 * 1.1 = 75900000 uusd
  const debtResponse = await terra.wasm.contractQuery(redBank, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "42000000",
      },
      {
        denom: "uusd",
        amount: "69000000",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 3. Repay, Pt. 1
//----------------------------------------------------------------------------------------

async function testRepay1() {
  process.stdout.write("Should handle repaying LUNA... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(
      user.key.accAddress,
      redBank,
      {
        repay_native: {
          denom: "uluna",
        },
      },
      { uluna: 12345678 }
    ),
  ]);

  // 42000000 - 12345678 = 29654322 uluna
  const debtResponse = await terra.wasm.contractQuery(redBank, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "29654322",
      },
      {
        denom: "uusd",
        amount: "69000000",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 4. Repay, Pt. 2
//----------------------------------------------------------------------------------------

async function testRepay2() {
  process.stdout.write("Should handle repaying UST... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(
      user.key.accAddress,
      redBank,
      {
        repay_native: {
          denom: "uusd",
        },
      },
      { uusd: 8888888 }
    ),
  ]);

  // 69000000 - 8888888 = 60111112 uusd
  const debtResponse = await terra.wasm.contractQuery(redBank, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "29654322",
      },
      {
        denom: "uusd",
        amount: "60111112",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 5. Set Debt
//----------------------------------------------------------------------------------------

async function testSetDebt() {
  process.stdout.write("Should forcibly set debt amount... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, redBank, {
      set_debt: { user: user.key.accAddress, denom: "uusd", amount: "69420" },
    }),
  ]);

  const debtResponse = await terra.wasm.contractQuery(redBank, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "29654322",
      },
      {
        denom: "uusd",
        amount: "69420",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user.key.accAddress)} as user`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Mock Mars"));

  await testBorrow1();
  await testBorrow2();
  await testRepay1();
  await testRepay2();
  await testSetDebt();

  console.log("");
})();
