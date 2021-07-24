import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployMockMirror, deployTerraswapPair, deployTerraswapToken } from "./fixture";
import { queryTokenBalance, sendTransaction, toEncodedBinary } from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let mirrorToken: string;
let mAssetToken: string;
let terraswapPair: string;
let terraswapLpToken: string;
let mirrorStaking: string;

async function setupTest() {
  let { cw20CodeId, cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock Mirror Token",
    "MIR"
  );
  mirrorToken = cw20Token;

  ({ cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock mAsset Token",
    "mASSET",
    6,
    cw20CodeId
  ));
  mAssetToken = cw20Token;

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(
    terra,
    deployer,
    cw20CodeId,
    mAssetToken
  ));

  mirrorStaking = await deployMockMirror(
    terra,
    deployer,
    mirrorToken,
    mAssetToken,
    terraswapLpToken
  );

  process.stdout.write("Fund contract with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirrorToken, {
      mint: {
        recipient: mirrorStaking,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user with mAsset... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mAssetToken, {
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
    new MsgExecuteContract(user.key.accAddress, mAssetToken, {
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
              amount: "420000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
            {
              amount: "69000000",
              info: {
                token: {
                  contract_addr: mAssetToken,
                },
              },
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

async function testBond() {
  process.stdout.write("Should bond LP tokens... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, terraswapLpToken, {
      send: {
        contract: mirrorStaking,
        amount: "170235131",
        msg: toEncodedBinary({
          bond: {
            asset_token: mAssetToken,
          },
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
    mirrorStaking,
    terraswapLpToken
  );
  expect(contractLpTokenBalance).to.equal("170235131");

  console.log(chalk.green("Passed!"));
}

async function testQueryRewardInfo1() {
  process.stdout.write("Should return correct reward info... ");

  const response = await terra.wasm.contractQuery(mirrorStaking, {
    reward_info: {
      staker_addr: user.key.accAddress,
      asset_token: mAssetToken,
    },
  });
  expect(response).to.deep.equal({
    staker_addr: user.key.accAddress,
    reward_infos: [
      {
        asset_token: mAssetToken,
        bond_amount: "170235131",
        pending_reward: "1000000",
        is_short: false,
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

async function testQueryRewardInfo2() {
  process.stdout.write("Should return zero reward for users who have no stake... ");

  const randomAddress = "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u";

  const response = await terra.wasm.contractQuery(mirrorStaking, {
    reward_info: {
      staker_addr: randomAddress,
      asset_token: mAssetToken,
    },
  });
  expect(response).to.deep.equal({
    staker_addr: randomAddress,
    reward_infos: [], // should return empty array, instead of throwing error
  });

  console.log(chalk.green("Passed!"));
}

async function testWithdraw() {
  process.stdout.write("Should withdraw reward... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirrorStaking, {
      withdraw: {
        asset_token: mAssetToken,
      },
    }),
  ]);

  const userMirBalance = await queryTokenBalance(terra, user.key.accAddress, mirrorToken);
  expect(userMirBalance).to.equal("1000000");

  console.log(chalk.green("Passed!"));
}

async function testUnbond() {
  process.stdout.write("Should unbond LP tokens... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirrorStaking, {
      unbond: {
        asset_token: mAssetToken,
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
    mirrorStaking,
    terraswapLpToken
  );
  expect(contractLpTokenBalance).to.equal("46778342");

  console.log(chalk.green("Passed!"));
}

(async () => {
  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user.key.accAddress)} as user`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: Mock Mirror Staking"));

  await testBond();
  await testQueryRewardInfo1();
  await testQueryRewardInfo2();
  await testWithdraw();
  await testUnbond();

  console.log("");
})();
