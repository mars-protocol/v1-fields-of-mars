import chalk from "chalk";
import { LocalTerra, MsgExecuteContract } from "@terra-money/terra.js";
import { expect } from "chai";
import {
  deployCw20Token,
  deployAstroportFactory,
  deployAstroportPair,
  deployOracle,
} from "./fixture";
import { sendTransaction, encodeUtf8 } from "./helpers";
import { SimulationResponse } from "./types";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user = terra.wallets.test2;

let testToken: string;
let astroportFactory: string;
let astroportPair: string;
let oracle: string;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  let { cw20CodeId, address } = await deployCw20Token(terra, deployer);
  testToken = address;

  ({ astroportFactory } = await deployAstroportFactory(terra, deployer, cw20CodeId));

  ({ astroportPair } = await deployAstroportPair(terra, deployer, astroportFactory, testToken));

  ({ oracle } = await deployOracle(terra, deployer));

  process.stdout.write("Setting asset: Fixed...");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
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
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          cw20: {
            contract_addr: testToken,
          },
        },
        price_source: {
          astroport_spot: {
            pair_address: astroportPair,
            asset_address: testToken,
          },
        },
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Fund user with tokens... ");

  await sendTransaction(terra, deployer, [
    new MsgExecuteContract(deployer.key.accAddress, testToken, {
      mint: {
        recipient: user.key.accAddress,
        amount: "694200000000",
      },
    }),
  ]);

  console.log(chalk.green("Done!"));

  process.stdout.write("Providing liquidity to Astroport pair... ");

  await sendTransaction(terra, user, [
    new MsgExecuteContract(user.key.accAddress, testToken, {
      increase_allowance: {
        amount: "694200000000",
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
                native_token: {
                  denom: "uusd",
                },
              },
              amount: "888888000000",
            },
            {
              info: {
                token: {
                  contract_addr: testToken,
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

  const response: string = await terra.wasm.contractQuery(oracle, {
    asset_price_by_reference: {
      asset_reference: encodeUtf8("uusd"),
    },
  });

  expect(response).to.equal("12345");

  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 2. Spot Price
//--------------------------------------------------------------------------------------------------

async function testSpotPrice() {
  process.stdout.write("2. Spot price... ");

  const response: string = await terra.wasm.contractQuery(oracle, {
    asset_price_by_reference: {
      asset_reference: encodeUtf8(testToken),
    },
  });

  const simulation: SimulationResponse = await terra.wasm.contractQuery(astroportPair, {
    simulation: {
      offer_asset: {
        info: {
          token: {
            contract_addr: testToken,
          },
        },
        amount: "1000000",
      },
    },
  });

  const price =
    (parseInt(simulation.return_amount) + parseInt(simulation.commission_amount)) / 1000000;

  expect(response).to.equal(price.toString());

  console.log(chalk.green("Done!"));
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

  await testFixedPrice();
  await testSpotPrice();

  console.log("");
})();
