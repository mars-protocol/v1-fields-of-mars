import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployCw20Token } from "./fixture";
import { queryCw20Balance, sendTransaction } from "./helpers";
import { Contract } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;
const user2 = terra.wallets.test3;

let cw20Token: Contract;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  cw20Token = await deployCw20Token(terra, deployer);
}

//--------------------------------------------------------------------------------------------------
// Test 1. Config
//--------------------------------------------------------------------------------------------------

async function testConfig() {
  process.stdout.write("1. Config... ");

  const tokenInfoResponse = await terra.wasm.contractQuery(cw20Token.address, {
    token_info: {},
  });

  expect(tokenInfoResponse).to.deep.equal({
    name: "Test Token",
    symbol: "TEST",
    decimals: 6,
    total_supply: "0",
  });

  const minterResponse = await terra.wasm.contractQuery(cw20Token.address, {
    minter: {},
  });

  expect(minterResponse).to.deep.equal({
    minter: deployer.key.accAddress,
    cap: null,
  });

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Mint
//--------------------------------------------------------------------------------------------------

async function testMint() {
  process.stdout.write("2. Mint... ");

  // Mint 88888 tokens to user1. The transaction needs to be sent by Minter
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, cw20Token.address, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "88888000000",
      },
    }),
  ]);

  // Check if user1 has the correct balance
  const balance = await queryCw20Balance(terra, user1.key.accAddress, cw20Token.address);
  expect(balance).to.equal("88888000000");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 3. Transfer
//--------------------------------------------------------------------------------------------------

async function testTransfer() {
  process.stdout.write("3. Transfer... ");

  // Transfer 69420 tokens from user1 to user2
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, cw20Token.address, {
      transfer: {
        recipient: user2.key.accAddress,
        amount: "69420000000",
      },
    }),
  ]);

  // Check user1's balance. Should be 88888 - 69420 = 19468
  const user1balance = await queryCw20Balance(terra, user1.key.accAddress, cw20Token.address);
  expect(user1balance).to.equal("19468000000");

  // Check user2's balance. Should be 69420
  const user2balance = await queryCw20Balance(terra, user2.key.accAddress, cw20Token.address);
  expect(user2balance).to.equal("69420000000");

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

  console.log(chalk.yellow("\nTest: CW20"));

  await testConfig();
  await testMint();
  await testTransfer();

  console.log("");
})();