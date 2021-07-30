import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployMockAnchor, deployTerraswapPair, deployTerraswapToken } from "./fixture";
import { queryTokenBalance, sendTransaction, toEncodedBinary } from "./helpers";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let anchorToken: string;
let anchorStaking: string;
let terraswapPair: string;
let terraswapLpToken: string;

//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function setupTest() {
  let { cw20CodeId, cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock Anchor Token",
    "ANC"
  );
  anchorToken = cw20Token;

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(terra, deployer, {
    asset_infos: [
      {
        native_token: {
          denom: "uusd",
        },
      },
      {
        token: {
          contract_addr: anchorToken,
        },
      },
    ],
    token_code_id: cw20CodeId,
  }));

  anchorStaking = await deployMockAnchor(terra, deployer, anchorToken, terraswapLpToken);

  process.stdout.write("Fund staking contract with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: anchorStaking,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user with ANC... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: user.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Provide liquidity to TerraSwap Pair... ");

  // Provide 69 mASSET + 420 UST
  // Should receive sqrt(69 * 420) = 170.235131 LP tokens
  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      user.key.accAddress,
      terraswapPair,
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
                  contract_addr: anchorToken,
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

//----------------------------------------------------------------------------------------
// Test 1. Bond
//----------------------------------------------------------------------------------------

async function testBond() {
  process.stdout.write("Should bond LP tokens... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, terraswapLpToken, {
      send: {
        contract: anchorStaking,
        amount: "170235131",
        msg: toEncodedBinary({
          bond: {},
        }),
      },
    }),
  ]);

  const userLpTokenBalance = await queryTokenBalance(
    terra,
    user.key.accAddress,
    terraswapLpToken
  );
  expect(userLpTokenBalance).to.equal("0");

  const contractLpTokenBalance = await queryTokenBalance(
    terra,
    anchorStaking,
    terraswapLpToken
  );
  expect(contractLpTokenBalance).to.equal("170235131");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 1. Query Staker Info, Pt. 1
//----------------------------------------------------------------------------------------

async function testQueryStakerInfo1() {
  process.stdout.write("Should return correct staker info... ");

  const stakerInfoResponse = await terra.wasm.contractQuery(anchorStaking, {
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

//----------------------------------------------------------------------------------------
// Test 2. Query Staker Info, Pt. 2
//----------------------------------------------------------------------------------------

async function testQueryStakerInfo2() {
  process.stdout.write("Should return zero for users who has no stake... ");

  const randomAddress = "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u";

  const stakerInfoResponse = await terra.wasm.contractQuery(anchorStaking, {
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

//----------------------------------------------------------------------------------------
// Test 3. Withdraw Reward
//----------------------------------------------------------------------------------------

async function testWithdraw() {
  process.stdout.write("Should withdraw reward... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchorStaking, {
      withdraw: {},
    }),
  ]);

  const userAncBalance = await queryTokenBalance(terra, user.key.accAddress, anchorToken);
  expect(userAncBalance).to.equal("1000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 4. Unbond
//----------------------------------------------------------------------------------------

async function testUnbond() {
  process.stdout.write("Should unbond LP tokens... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, anchorStaking, {
      unbond: {
        amount: "123456789",
      },
    }),
  ]);

  const userLpTokenBalance = await queryTokenBalance(
    terra,
    user.key.accAddress,
    terraswapLpToken
  );
  expect(userLpTokenBalance).to.equal("123456789");

  // 170235131 - 123456789 = 46778342
  const contractLpTokenBalance = await queryTokenBalance(
    terra,
    anchorStaking,
    terraswapLpToken
  );
  expect(contractLpTokenBalance).to.equal("46778342");

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

  console.log(chalk.yellow("\nTest: Mock Anchor Staking"));

  await testBond();
  await testQueryStakerInfo1();
  await testQueryStakerInfo2();
  await testWithdraw();
  await testUnbond();

  console.log("");
})();
