import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployCw20Token, deployAstroport, deployAnchorStaking } from "./fixture";
import { queryCw20Balance, sendTransaction, toEncodedBinary } from "./helpers";
import { Protocols } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let anchor: Protocols.Anchor;
let astroport: Protocols.Astroport;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  const token = await deployCw20Token(terra, deployer);
  astroport = await deployAstroport(terra, deployer, token);
  const staking = await deployAnchorStaking(terra, deployer, token, astroport);
  anchor = { token, staking };

  process.stdout.write("Fund staking contract with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: anchor.staking.address,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchor.token.address, {
      mint: {
        recipient: user.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Provide liquidity to Astroport Pair... ");

  // Provide 69 ANC + 420 UST
  // Should receive sqrt(69 * 420) = 170.235131 LP tokens
  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchor.token.address, {
      increase_allowance: {
        amount: "69000000",
        spender: astroport.pair.address,
      },
    }),
    new MsgExecuteContract(
      user.key.accAddress,
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
              amount: "420000000",
            },
            {
              info: {
                token: {
                  contract_addr: anchor.token.address,
                },
              },
              amount: "69000000",
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

//--------------------------------------------------------------------------------------------------
// Test 1. Bond
//--------------------------------------------------------------------------------------------------

async function testBond() {
  process.stdout.write("1. Bond... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, astroport.shareToken.address, {
      send: {
        contract: anchor.staking.address,
        amount: "170235131",
        msg: toEncodedBinary({
          bond: {},
        }),
      },
    }),
  ]);

  const userLpTokenBalance = await queryCw20Balance(
    terra,
    user.key.accAddress,
    astroport.shareToken.address
  );
  expect(userLpTokenBalance).to.equal("0");

  const contractLpTokenBalance = await queryCw20Balance(
    terra,
    anchor.staking.address,
    astroport.shareToken.address
  );
  expect(contractLpTokenBalance).to.equal("170235131");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Query Staker Info, Pt. 1
//--------------------------------------------------------------------------------------------------

async function testQueryStakerInfo1() {
  process.stdout.write("2. Query staker info, part 1... ");

  const stakerInfoResponse = await terra.wasm.contractQuery(anchor.staking.address, {
    staker_info: {
      staker: user.key.accAddress,
      block_height: undefined,
    },
  });
  expect(stakerInfoResponse).to.deep.equal({
    staker: user.key.accAddress,
    reward_index: "0",
    bond_amount: "170235131",
    pending_reward: "1000000",
  });

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Query Staker Info, Pt. 2
//--------------------------------------------------------------------------------------------------

async function testQueryStakerInfo2() {
  process.stdout.write("3. Query staker info, part 2... ");

  const randomAddress = "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u";

  const stakerInfoResponse = await terra.wasm.contractQuery(anchor.staking.address, {
    staker_info: {
      staker: randomAddress,
      block_height: undefined,
    },
  });
  expect(stakerInfoResponse).to.deep.equal({
    staker: randomAddress,
    reward_index: "0",
    bond_amount: "0", // should be zero here instead of throwing an error
    pending_reward: "1000000", // contract returns 100000 regardless
  });

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 3. Withdraw Reward
//--------------------------------------------------------------------------------------------------

async function testWithdraw() {
  process.stdout.write("4. Withdraw reward... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchor.staking.address, {
      withdraw: {},
    }),
  ]);

  const userAncBalance = await queryCw20Balance(terra, user.key.accAddress, anchor.token.address);
  expect(userAncBalance).to.equal("1000000");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 4. Unbond
//--------------------------------------------------------------------------------------------------

async function testUnbond() {
  process.stdout.write("5. Unbond... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchor.staking.address, {
      unbond: {
        amount: "123456789",
      },
    }),
  ]);

  const userLpTokenBalance = await queryCw20Balance(
    terra,
    user.key.accAddress,
    astroport.shareToken.address
  );
  expect(userLpTokenBalance).to.equal("123456789");

  // 170235131 - 123456789 = 46778342
  const contractLpTokenBalance = await queryCw20Balance(
    terra,
    anchor.staking.address,
    astroport.shareToken.address
  );
  expect(contractLpTokenBalance).to.equal("46778342");

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

  console.log(chalk.yellow("\nTest: Mock Anchor Staking"));

  await testBond();
  await testQueryStakerInfo1();
  await testQueryStakerInfo2();
  await testWithdraw();
  await testUnbond();

  console.log("");
})();
