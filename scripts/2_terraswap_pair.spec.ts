import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployTerraswapPair, deployTerraswapToken } from "./fixture";
import {
  queryNativeTokenBalance,
  queryTokenBalance,
  sendTransaction,
  toEncodedBinary,
} from "./helpers";

//----------------------------------------------------------------------------------------
// Variables
//----------------------------------------------------------------------------------------

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;
const user2 = terra.wallets.test3;

let cw20CodeId: number;
let cw20Token: string;
let terraswapPair: string;
let terraswapLpToken: string;

//----------------------------------------------------------------------------------------
// Setup
//----------------------------------------------------------------------------------------

async function setupTest() {
  ({ cw20CodeId, cw20Token } = await deployTerraswapToken(
    terra,
    deployer,
    "Mock Mirror Token",
    "MIR"
  ));

  ({ terraswapPair, terraswapLpToken } = await deployTerraswapPair(terra, deployer, {
    asset_infos: [
      { native_token: { denom: "uusd" } },
      { token: { contract_addr: cw20Token } },
    ],
    token_code_id: cw20CodeId,
  }));
}

//----------------------------------------------------------------------------------------
// Test 1. Provide Initial Liquidity
//----------------------------------------------------------------------------------------

async function testProvideInitialLiquidity() {
  process.stdout.write("Should handle providing initial liquidity... ");

  // Fist, mint some MIR tokens to the users
  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, cw20Token, {
      mint: {
        recipient: user1.key.accAddress,
        amount: "10000000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, cw20Token, {
      mint: {
        recipient: user2.key.accAddress,
        amount: "10000000000",
      },
    }),
  ]);

  // User1 provides 100 MIR + 1000 UST (price: 1 MIR = 10 UST)
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, cw20Token, {
      increase_allowance: {
        amount: "100000000",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      terraswapPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "1000000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
            {
              amount: "100000000",
              info: {
                token: {
                  contract_addr: cw20Token,
                },
              },
            },
          ],
        },
      },
      {
        uusd: "1000000000",
      }
    ),
  ]);

  // The liquidity pool should have 1000 UST balance
  const ustBalance = await queryNativeTokenBalance(terra, terraswapPair);
  expect(ustBalance).to.equal("1000000000");

  // The liquidity pool should have 100 MIR balance
  const tstBalance = await queryTokenBalance(terra, terraswapPair, cw20Token);
  expect(tstBalance).to.equal("100000000");

  // User1 should receive sqrt(100e6 * 1000e6) = 316.227766 LP tokens
  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("316227766");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 2. Provide Further Liquidity
//----------------------------------------------------------------------------------------

async function testProvideFurtherLiquidity() {
  process.stdout.write("Should handle providing further liquidity... ");

  // User provides another 69 MIR
  // The amount of UST needed: 69e6 * 1000e6 / 100e6 = 690000000
  await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, cw20Token, {
      increase_allowance: {
        amount: "69000000",
        spender: terraswapPair,
      },
    }),
    new MsgExecuteContract(
      user1.key.accAddress,
      terraswapPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "690000000",
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
                  contract_addr: cw20Token,
                },
              },
            },
          ],
        },
      },
      {
        uusd: "690000000",
      }
    ),
  ]);

  // The amount of LP token the user should receive is:
  // min(ustDeposit * totalShare / ustBalance, mirDeposit * totalShare / tstBalance)
  // = min(690e6 * 316227766 / 1000e6, 69e6 * 316227766 / 100e6)
  // = min(218197158, 218197158) = 218197158
  //
  // Total LP token user1 should have at this point:
  // 316227766 + 218197158 = 534424924
  const lpTokenBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    terraswapLpToken
  );
  expect(lpTokenBalance).to.equal("534424924");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 3. Swap
//----------------------------------------------------------------------------------------

async function testSwap() {
  process.stdout.write("Should handle swaps... ");

  // User2 dumps 100 MIR for UST
  //
  // The trade amounts are calculated as follows
  //
  // kValueBefore = poolUstBalance * pooltstBalance
  // = 1690000000 * 169000000 = 285610000000000000;
  //
  // returnUstAmount = poolUstBalance - kValueBefore / (pooltstBalance + sendMirAmount)
  // = 1690000000 - 285610000000000000 / (169000000 + 100000000)
  // = 628252788
  //
  // fee = returnUstAmount * feeRate
  // = 628252789 * 0.003
  // = 1884758
  //
  // returnUstAmountAfterFee = returnUstAmount - fee
  // = 628252788 - 1884758
  // = 626368030
  //
  // The user should receive 626.368030 UST minus tax; tax is calculated as follows:
  // Github: terraswap/terraswap/packages/terraswap/src/asset.rs#L44
  //
  // tax = std::cmp::min(
  //   amount - amount.multiply_ratio(1e18, 1e18 * tax_rate + 1e18),
  //   tax_cap
  // );
  //
  // Default values for LocalTerra:
  //
  // tax_rate = 0.001
  // tax_cap = 1000000 uusd
  //
  await sendTransaction(terra, user2, [
    new MsgExecuteContract(user2.key.accAddress, cw20Token, {
      send: {
        amount: "100000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          swap: {},
        }),
      },
    }),
  ]);

  // The pool should send out 626.368030 UST
  // total balance: 1690.000000 - 626.368030 = 1063631970 uusd
  const poolUstBalance = await queryNativeTokenBalance(terra, terraswapPair);
  expect(poolUstBalance).to.equal("1063631970");

  // The pool should receive 100 MIR
  // total balance: 169 + 100 = 269 MIR
  const pooltstBalance = await queryTokenBalance(terra, terraswapPair, cw20Token);
  expect(pooltstBalance).to.equal("269000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Test 4. Remove Liquidity
//----------------------------------------------------------------------------------------

async function testRemoveLiquidity() {
  process.stdout.write("Should handle removal of liquidity... ");

  // User1 removes 420 LP tokens
  //
  // Prior to withdrawal, the pair contract has 534.424924 LP token supply, 269 MIR, and
  // 1063.631970 UST
  //
  // Burning 420 LP token should get the user:
  // 269 * 420 / 534.424924 = 211.404810 MIR
  // 1063.631970 * 420 / 534.424924 = 83.5899314 UST
  //
  const result = await sendTransaction(terra, user1, [
    new MsgExecuteContract(user1.key.accAddress, terraswapLpToken, {
      send: {
        amount: "420000000",
        contract: terraswapPair,
        msg: toEncodedBinary({
          withdraw_liquidity: {},
        }),
      },
    }),
  ]);

  // Due to the tax charged on UST transfers, it's difficult to estimate exactly how much
  // UST the user should receive in his wallet. Therefore we simply validate the amounts
  // recorded in the transaction's log message.
  const refundAssets = result.logs[0].events[6].attributes.find((attr) => {
    if (attr.key === "refund_assets") return true;
    else return false;
  });
  expect(refundAssets?.value).to.equal(`835899314uusd, 211404810${cw20Token}`);

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// Main
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Info"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(user1.key.accAddress)} as user 1`);
  console.log(`Use ${chalk.cyan(user2.key.accAddress)} as user 2`);

  console.log(chalk.yellow("\nTest: Setup"));

  await setupTest();

  console.log(chalk.yellow("\nTest: TerraSwap Pair"));

  await testProvideInitialLiquidity();
  await testProvideFurtherLiquidity();
  await testSwap();
  await testRemoveLiquidity();

  console.log("");
})();
