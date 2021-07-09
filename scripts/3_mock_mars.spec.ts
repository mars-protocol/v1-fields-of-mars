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

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let mars: string;

async function setupTest() {
  mars = await deployMockMars(terra, deployer, "1.1");

  process.stdout.write("Fund contract with LUNA and UST...");

  await sendTransaction(terra, deployer, [
    new MsgSend(
      deployer.key.accAddress,
      mars,
      { uluna: 100000000, uusd: 100000000 } // fund contract with 100 LUNA + 100 UST
    ),
  ]);

  console.log(chalk.green("Done!"));
}

async function testBorrow1() {
  process.stdout.write("Should handle borrowing LUNA... ");

  const userLunaBalanceBefore = await queryNativeTokenBalance(
    terra,
    user.key.accAddress,
    "uluna"
  );

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mars, {
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

  // Should be 42000000 * 1.1 = 46200000 uluna
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "46200000",
      },
      {
        denom: "uusd",
        amount: "0",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testBorrow2() {
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
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "46200000",
      },
      {
        denom: "uusd",
        amount: "75900000",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testRepay1() {
  process.stdout.write("Should handle repaying LUNA... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(
      user.key.accAddress,
      mars,
      {
        repay_native: {
          denom: "uluna",
        },
      },
      { uluna: 12345678 }
    ),
  ]);

  // User pays 12.345678 LUNA. At mockInterestRate = 1.1, this should reduce the debt by
  // 12345678 / 1.1 = 11223343 uluna
  // Remaining debt = (42000000 - 11223343) * 1.1
  // = 30776657 * 1.1
  // = 33854322
  // 46200000 - 12345678 = 33854322 (match)
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "33854322",
      },
      {
        denom: "uusd",
        amount: "75900000",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testRepay2() {
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
      { uusd: 8888888 }
    ),
  ]);

  // User pays 8.888888 UST. At mockInterestRate = 1.1, this should reduce the debt by
  // 8888888 / 1.1 = 8080807 uluna
  // Remaining debt = (69000000 - 8080807) * 1.1
  // = 60919193 * 1.1
  // = 67011112
  // 75900000 - 8888888 = 67011112 (match)
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "33854322",
      },
      {
        denom: "uusd",
        amount: "67011112",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testMigrate() {
  process.stdout.write("Should migrate... ");

  // We don't actually change the code ID during migration
  const codeId = (await terra.wasm.contractInfo(mars)).code_id;

  await sendTransaction(terra, deployer, [
    new MsgMigrateContract(deployer.key.accAddress, mars, codeId, {
      mock_interest_rate: "1.2",
    }),
  ]);

  // With the new mockInterestRate, debt amounts should be:
  // uluna: 30776657 * 1.2 = 36931988
  // uusd: 60919193 * 1.2 = 73103031
  const debtResponse = await terra.wasm.contractQuery(mars, {
    debt: {
      address: user.key.accAddress,
    },
  });
  expect(debtResponse).to.deep.equal({
    debts: [
      {
        denom: "uluna",
        amount: "36931988",
      },
      {
        denom: "uusd",
        amount: "73103031",
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

(async () => {
  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Mock Mars"));

  await testBorrow1();
  await testBorrow2();
  await testRepay1();
  await testRepay2();
  await testMigrate();

  console.log("");
})();
