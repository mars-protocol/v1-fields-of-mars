import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployCw20Token, deployAstroport, deployMirrorStaking } from "./fixture";
import { queryCw20Balance, sendTransaction, toEncodedBinary } from "./helpers";
import { Protocols } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let mirror: Protocols.Mirror;
let astroport: Protocols.Astroport;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  const mirrorToken = await deployCw20Token(terra, deployer);
  const mAssetToken = await deployCw20Token(terra, deployer, mirrorToken.codeId);
  astroport = await deployAstroport(terra, deployer, mAssetToken);
  const staking = await deployMirrorStaking(terra, deployer, mirrorToken);
  mirror = { token: mirrorToken, mAsset: mAssetToken, staking };

  process.stdout.write("Registering asset... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirror.staking.address, {
      register_asset: {
        asset_token: mirror.mAsset.address,
        staking_token: astroport.liquidityToken.address,
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund contract with MIR... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirror.token.address, {
      mint: {
        recipient: mirror.staking.address,
        amount: "100000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user with mAsset... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, mirror.mAsset.address, {
      mint: {
        recipient: user.key.accAddress,
        amount: "69000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Provide liquidity to Astroport Pair... ");

  // Provide 69 mASSET + 420 UST
  // Should receive sqrt(69 * 420) = 170.235131 LP tokens
  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirror.mAsset.address, {
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
                  contract_addr: mirror.mAsset.address,
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
    new MsgExecuteContract(user.key.accAddress, astroport.liquidityToken.address, {
      send: {
        contract: mirror.staking.address,
        amount: "170235131",
        msg: toEncodedBinary({
          bond: {
            asset_token: mirror.mAsset.address,
          },
        }),
      },
    }),
  ]);

  const userLpTokenBalance = await queryCw20Balance(
    terra,
    user.key.accAddress,
    astroport.liquidityToken.address
  );

  expect(userLpTokenBalance).to.equal("0");

  const contractLpTokenBalance = await queryCw20Balance(
    terra,
    mirror.staking.address,
    astroport.liquidityToken.address
  );

  expect(contractLpTokenBalance).to.equal("170235131");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Query Reward Info, Pt. 1
//--------------------------------------------------------------------------------------------------

async function testQueryRewardInfo1() {
  process.stdout.write("2. Query reward info, part 1... ");

  const response = await terra.wasm.contractQuery(mirror.staking.address, {
    reward_info: {
      staker_addr: user.key.accAddress,
      asset_token: mirror.mAsset.address,
    },
  });

  expect(response).to.deep.equal({
    staker_addr: user.key.accAddress,
    reward_infos: [
      {
        asset_token: mirror.mAsset.address,
        bond_amount: "170235131",
        pending_reward: "1000000",
        is_short: false,
      },
    ],
  });

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 3. Query Reward Info, Pt. 2
//--------------------------------------------------------------------------------------------------

async function testQueryRewardInfo2() {
  process.stdout.write("3. Query reward info, part 2... ");

  const randomAddress = "terra10llyp6v3j3her8u3ce66ragytu45kcmd9asj3u";

  const response = await terra.wasm.contractQuery(mirror.staking.address, {
    reward_info: {
      staker_addr: randomAddress,
      asset_token: mirror.mAsset.address,
    },
  });

  expect(response).to.deep.equal({
    staker_addr: randomAddress,
    reward_infos: [], // should return empty array, instead of throwing error
  });

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 4. Withdraw Reward
//--------------------------------------------------------------------------------------------------

async function testWithdraw() {
  process.stdout.write("4. Withdraw reward... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirror.staking.address, {
      withdraw: {
        asset_token: mirror.mAsset.address,
      },
    }),
  ]);

  const userMirBalance = await queryCw20Balance(terra, user.key.accAddress, mirror.token.address);
  expect(userMirBalance).to.equal("1000000");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Test 5. Unbond
//--------------------------------------------------------------------------------------------------

async function testUnbond() {
  process.stdout.write("5. Unbond... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, mirror.staking.address, {
      unbond: {
        asset_token: mirror.mAsset.address,
        amount: "123456789",
      },
    }),
  ]);

  const userLpTokenBalance = await queryCw20Balance(
    terra,
    user.key.accAddress,
    astroport.liquidityToken.address
  );

  expect(userLpTokenBalance).to.equal("123456789");

  // 170235131 - 123456789 = 46778342
  const contractLpTokenBalance = await queryCw20Balance(
    terra,
    mirror.staking.address,
    astroport.liquidityToken.address
  );

  expect(contractLpTokenBalance).to.equal("46778342");

  console.log(chalk.green("Passed!"));
}

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

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
