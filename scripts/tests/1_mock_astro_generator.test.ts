import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import {
  deployCw20Token,
  deployAstroportFactory,
  deployAstroportPair,
  deployAstroGenerator,
} from "./fixture";
import { encodeBase64 } from "../helpers/encoding";
import { queryCw20Balance } from "../helpers/queries";
import { sendTransaction } from "../helpers/tx";
import { PendingTokenResponse } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let testToken: string;
let astroportFactory: string;
let astroportPair: string;
let astroportLpToken: string;
let astroToken: string;
let astroGenerator: string;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  const { cw20CodeId, address } = await deployCw20Token(deployer);
  testToken = address;

  const result = await deployCw20Token(deployer, cw20CodeId, "Astroport Token", "ASTRO");
  astroToken = result.address;

  ({ astroportFactory } = await deployAstroportFactory(deployer, cw20CodeId));

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      native_token: {
        denom: "uusd",
      },
    },
    {
      token: {
        contract_addr: testToken,
      },
    },
  ]));

  ({ astroGenerator } = await deployAstroGenerator(
    deployer,
    astroportLpToken,
    astroToken,
    testToken
  ));

  process.stdout.write("Minting TEST token to user... ");

  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, testToken, {
      mint: {
        recipient: user.key.accAddress,
        amount: "69420000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Minting TEST token to generator... ");

  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, testToken, {
      mint: {
        recipient: astroGenerator,
        amount: "10000000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Minting ASTRO token to generator... ");

  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: astroGenerator,
        amount: "10000000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("User providing TEST + UST liquidity to Astroport pair... ");

  // user should receive sqrt(69420000000 * 88888000000) = 78553198279 liquidity tokens
  await sendTransaction(user, [
    new MsgExecuteContract(user.key.accAddress, testToken, {
      increase_allowance: {
        amount: "69420000000",
        spender: astroportPair,
      },
    }),
    new MsgExecuteContract(
      user.key.accAddress,
      astroportPair,
      {
        provide_liquidity: {
          assets: [
            {
              info: {
                token: {
                  contract_addr: testToken,
                },
              },
              amount: "69420000000",
            },
            {
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "88888000000",
            },
          ],
        },
      },
      {
        uusd: "88888000000",
      }
    ),
  ]);

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Deposit liquidity tokens
//--------------------------------------------------------------------------------------------------

async function testDeposit() {
  process.stdout.write("1. Deposit liquidity tokens... ");

  const userDepositBefore: string = await terra.wasm.contractQuery(astroGenerator, {
    deposit: {
      lp_token: astroportLpToken,
      user: user.key.accAddress,
    },
  });

  expect(userDepositBefore).to.equal("0");

  await sendTransaction(user, [
    new MsgExecuteContract(user.key.accAddress, astroportLpToken, {
      send: {
        contract: astroGenerator,
        amount: "78553198279",
        msg: encodeBase64({
          deposit: {},
        }),
      },
    }),
  ]);

  const userDepositAfter: string = await terra.wasm.contractQuery(astroGenerator, {
    deposit: {
      lp_token: astroportLpToken,
      user: user.key.accAddress,
    },
  });

  expect(userDepositAfter).to.equal("78553198279");

  // user should have received reward as a result of the deposit
  const userAstroBalance = await queryCw20Balance(terra, user.key.accAddress, astroToken);

  expect(userAstroBalance).to.equal("1000000");

  const userTestBalance = await queryCw20Balance(terra, user.key.accAddress, testToken);

  expect(userTestBalance).to.equal("500000");

  console.log(chalk.green("Success!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Query and withdraw reward
//--------------------------------------------------------------------------------------------------

async function testReward() {
  process.stdout.write("2. Query and withdraw reward... ");

  // claim reward by sending a withdraw tx with zero amount
  await sendTransaction(user, [
    new MsgExecuteContract(user.key.accAddress, astroGenerator, {
      withdraw: {
        lp_token: astroportLpToken,
        amount: "0",
      },
    }),
  ]);

  const userAstroBalance = await queryCw20Balance(terra, user.key.accAddress, astroToken);

  expect(userAstroBalance).to.equal("2000000");

  const userTestBalance = await queryCw20Balance(terra, user.key.accAddress, testToken);

  expect(userTestBalance).to.equal("1000000");

  const pendingTokenResponse: PendingTokenResponse = await terra.wasm.contractQuery(
    astroGenerator,
    {
      pending_token: {
        lp_token: astroportLpToken,
        user: user.key.accAddress,
      },
    }
  );

  expect(pendingTokenResponse.pending).to.equal("1000000");
  expect(pendingTokenResponse.pending_on_proxy).to.equal("500000");

  console.log(chalk.green("Success!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Withdraw liquidity tokens
//--------------------------------------------------------------------------------------------------

async function testWithdraw() {
  process.stdout.write("3. Withdraw liquidity tokens... ");

  await sendTransaction(user, [
    new MsgExecuteContract(user.key.accAddress, astroGenerator, {
      withdraw: {
        lp_token: astroportLpToken,
        amount: "12345",
      },
    }),
  ]);

  const userDeposit: string = await terra.wasm.contractQuery(astroGenerator, {
    deposit: {
      lp_token: astroportLpToken,
      user: user.key.accAddress,
    },
  });

  expect(userDeposit).to.equal("78553185934"); // 78553198279 - 12345

  const userLpTokenBalance = await queryCw20Balance(terra, user.key.accAddress, astroportLpToken);

  expect(userLpTokenBalance).to.equal("12345");

  const userAstroBalance = await queryCw20Balance(terra, user.key.accAddress, astroToken);

  expect(userAstroBalance).to.equal("3000000");

  const userTestBalance = await queryCw20Balance(terra, user.key.accAddress, testToken);

  expect(userTestBalance).to.equal("1500000");

  console.log(chalk.green("Success!"));
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

  await testDeposit();
  await testReward();
  await testWithdraw();

  console.log("");
})();
