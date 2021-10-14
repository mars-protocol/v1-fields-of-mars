import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import { deployCw20Token, deployAstroport, deployOracle } from "./fixture";
import { sendTransaction, toUtf8Array, computeSwapOutput } from "./helpers";
import { Protocols, Contract, Oracle } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let cw20Token: Contract;
let astroport: Protocols.Astroport;
let oracle: Contract;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  cw20Token = await deployCw20Token(terra, deployer);
  astroport = await deployAstroport(terra, deployer, cw20Token);
  oracle = await deployOracle(terra, deployer);

  process.stdout.write("Setting asset: Fixed...");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle.address, {
      set_asset: {
        asset: {
          native: {
            denom: "uusd",
          },
        },
        price_source: {
          fixed: {
            price: "12345",
          },
        },
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Setting asset: Spot...");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle.address, {
      set_asset: {
        asset: {
          cw20: {
            contract_addr: cw20Token.address,
          },
        },
        price_source: {
          astroport_spot: {
            pair_address: astroport.pair.address,
            asset_address: cw20Token.address,
          },
        },
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user1 with tokens... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, cw20Token.address, {
      mint: {
        recipient: user.key.accAddress,
        amount: "694200000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Providing liquidity to Astroport pair... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, cw20Token.address, {
      increase_allowance: {
        amount: "694200000000",
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
              amount: "888888000000",
            },
            {
              info: {
                token: {
                  contract_addr: cw20Token.address,
                },
              },
              amount: "694200000000",
            },
          ],
        },
      },
      {
        uusd: "888888000000",
      }
    ),
  ]);

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Fixed Price
//--------------------------------------------------------------------------------------------------

async function testFixedPrice() {
  process.stdout.write("1. Fixed price... ");

  const response: Oracle.AssetPriceResponse = await terra.wasm.contractQuery(oracle.address, {
    asset_price_by_reference: {
      asset_reference: toUtf8Array("uusd"),
    },
  });

  expect(response.price).to.equal("12345");

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Spot Price
//--------------------------------------------------------------------------------------------------

async function testSpotPrice() {
  process.stdout.write("2. Spot price... ");

  const response: Oracle.AssetPriceResponse = await terra.wasm.contractQuery(oracle.address, {
    asset_price_by_reference: {
      asset_reference: toUtf8Array(cw20Token.address),
    },
  });

  const swapOutput = computeSwapOutput(1000000, 694200000000, 888888000000);
  const price = (parseInt(swapOutput.returnAmount) + parseInt(swapOutput.commission)) / 1000000;

  expect(response.price).to.equal(price.toString());

  console.log(chalk.green("Done!"));
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

  console.log(chalk.yellow("\nTest: Mock Oracle"));

  await testFixedPrice();
  await testSpotPrice();

  console.log("");
})();
