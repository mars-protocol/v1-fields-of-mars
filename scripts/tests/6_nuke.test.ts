import * as path from "path";
import BN from "bn.js";
import chalk from "chalk";
import { expect } from "chai";
import { LocalTerra, MsgExecuteContract, MsgMigrateContract, MsgSend } from "@terra-money/terra.js";
import { sendTransaction, storeCode } from "../helpers/tx";
import { encodeBase64 } from "../helpers/encoding";
import {
  deployCw20Token,
  deployAstroportFactory,
  deployAstroportPair,
  deployAstroGenerator,
  deployRedBank,
  deployOracle,
  deployMartianField,
} from "./fixture";
import { queryCw20Balance, queryNativeBalance } from "../helpers/queries";

// LocalTerra instance
const terra = new LocalTerra();

// User addresses
const deployer = terra.wallets.test1;
// for testing, we order the accounts alphabetically
const alice = terra.wallets.test3;   // terra1757tkx08n0cqrw7p86ny9lnxsqeth0wgp0em95
const bob = terra.wallets.test2;     // terra17lmam6zguazs5q5u6z5mmx76uj63gldnse2pdp
const charlie = terra.wallets.test4; // terra199vw7724lzkwz6lf2hsx04lrxfkz09tg8dlp6r

let anchorToken: string;
let astroToken: string;
let astroportFactory: string;
let ancUstPair: string;
let ancUstLpToken: string;
let astroUstPair: string;
let astroUstLpToken: string;
let astroGenerator: string;
let oracle: string;
let bank: string;
let field: string;

//--------------------------------------------------------------------------------------------------
// Setup
//--------------------------------------------------------------------------------------------------

async function setupTest() {
  const { cw20CodeId, address } = await deployCw20Token(deployer, undefined, "Anchor Token", "ANC");
  anchorToken = address;

  const result = await deployCw20Token(deployer, cw20CodeId, "Astroport Token", "ASTRO");
  astroToken = result.address;

  ({ astroportFactory } = await deployAstroportFactory(deployer, cw20CodeId));

  let { astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      token: {
        contract_addr: anchorToken,
      },
    },
    {
      native_token: {
        denom: "uusd",
      },
    },
  ]);
  ancUstPair = astroportPair;
  ancUstLpToken = astroportLpToken;

  ({ astroportPair, astroportLpToken } = await deployAstroportPair(deployer, astroportFactory, [
    {
      native_token: {
        denom: "uusd",
      },
    },
    {
      token: {
        contract_addr: astroToken,
      },
    },
  ]));
  astroUstPair = astroportPair;
  astroUstLpToken = astroportLpToken;

  ({ astroGenerator } = await deployAstroGenerator(
    deployer,
    ancUstLpToken,
    astroToken,
    anchorToken
  ));

  ({ oracle } = await deployOracle(deployer));

  ({ bank } = await deployRedBank(deployer));

  ({ field } = await deployMartianField(deployer, {
    primary_asset_info: {
      cw20: anchorToken,
    },
    secondary_asset_info: {
      native: "uusd",
    },
    astro_token_info: {
      cw20: astroToken,
    },
    primary_pair: {
      contract_addr: ancUstPair,
      liquidity_token: ancUstLpToken,
    },
    astro_pair: {
      contract_addr: astroUstPair,
      liquidity_token: astroUstLpToken,
    },
    astro_generator: {
      contract_addr: astroGenerator,
    },
    red_bank: {
      contract_addr: bank,
    },
    oracle: {
      contract_addr: oracle,
    },
    treasury: deployer.key.accAddress,
    governance: deployer.key.accAddress,
    operators: [deployer.key.accAddress],
    max_ltv: "0.65",
    fee_rate: "0",
    bonus_rate: "0.05",
  }));

  process.stdout.write("Configuring ANC and UST price oracle...");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          native: {
            denom: "uusd",
          },
        },
        price_source: {
          fixed: {
            price: "1",
          },
        },
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, oracle, {
      set_asset: {
        asset: {
          cw20: {
            contract_addr: anchorToken,
          },
        },
        price_source: {
          astroport_spot: {
            pair_address: ancUstPair,
            asset_address: anchorToken,
          },
        },
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund deployer and users with ANC... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: alice.key.accAddress,
        amount: "69000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: bob.key.accAddress,
        amount: "34500000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: charlie.key.accAddress,
        amount: "10000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund deployer with ASTRO... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: deployer.key.accAddress,
        amount: "1000000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Astro generator contract with ANC and ASTRO... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      mint: {
        recipient: astroGenerator,
        amount: "100000000",
      },
    }),
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      mint: {
        recipient: astroGenerator,
        amount: "100000000",
      },
    }),
  ]);
  console.log(chalk.green("Done!"));

  process.stdout.write("Fund Mars contract with UST...");
  await sendTransaction(deployer, [
    new MsgSend(deployer.key.accAddress, bank, { uusd: 100000000000 }),
  ]);
  console.log(chalk.green("Done!"));

  // deployer provides 69 ANC + 420 UST
  // should receive sqrt(69000000 * 420000000) = 170235131 uLP
  process.stdout.write("Provide initial liquidity to ANC-UST Pair... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: ancUstPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      ancUstPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "69000000",
              info: {
                token: {
                  contract_addr: anchorToken,
                },
              },
            },
            {
              amount: "420000000",
              info: {
                native_token: {
                  denom: "uusd",
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

  // deployer provides 100 ASTRO + 150 UST
  // should receive sqrt(100000000 * 150000000) = 122474487 uLP
  process.stdout.write("Provide initial liquidity to ASTRO-UST Pair... ");
  await sendTransaction(deployer, [
    new MsgExecuteContract(deployer.key.accAddress, astroToken, {
      increase_allowance: {
        amount: "100000000",
        spender: astroUstPair,
      },
    }),
    new MsgExecuteContract(
      deployer.key.accAddress,
      astroUstPair,
      {
        provide_liquidity: {
          assets: [
            {
              amount: "100000000",
              info: {
                token: {
                  contract_addr: astroToken,
                },
              },
            },
            {
              amount: "150000000",
              info: {
                native_token: {
                  denom: "uusd",
                },
              },
            },
          ],
        },
      },
      {
        uusd: "150000000",
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // alice opens a position
  // should receive sqrt(69000000 * 420000000) = 170235131 uLP
  // should get 170235131 * 1e6 = 170235131000000 bond units
  process.stdout.write("Alice opens a position... ");
  await sendTransaction(alice, [
    new MsgExecuteContract(alice.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "69000000",
        spender: field,
      },
    }),
    new MsgExecuteContract(
      alice.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                cw20: anchorToken,
              },
              amount: "69000000",
            },
          },
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "420000000",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005",
            },
          },
        ],
      },
      {
        uusd: 420000000,
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // bob opens a position
  // should receive sqrt(34500000 * 210000000) = 85117565 uLP
  // should get 85117565 * 1e6 = 85117565000000 bond units
  process.stdout.write("Bob opens a position... ");
  await sendTransaction(bob, [
    new MsgExecuteContract(bob.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "34500000",
        spender: field,
      },
    }),
    new MsgExecuteContract(
      bob.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                cw20: anchorToken,
              },
              amount: "34500000",
            },
          },
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "210000000",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005",
            },
          },
        ],
      },
      {
        uusd: 210000000,
      }
    ),
  ]);
  console.log(chalk.green("Done!"));

  // charlie opens a position
  // current pool depth:
  // uANC: 69000000 + 69000000 + 34500000 = 172500000
  // uusd: 420000000 + 420000000 + 210000000 = 1050000000
  // uLP:  170235131 + 170235131 + 85117565 = 425587827
  // amount of uusd charlie needs to deposit: 10000000 * 1050000000 / 172500000 = 60869565
  // amount of uLP to mint: min(425587827 * 10000000 / 172500000, 425587827 * 60869565 / 1050000000) = 24671757
  // amount of bond units: 24671757 * 1e6 = 24671757000000
  process.stdout.write("Charlie opens a position... ");
  await sendTransaction(charlie, [
    new MsgExecuteContract(charlie.key.accAddress, anchorToken, {
      increase_allowance: {
        amount: "10000000",
        spender: field,
      },
    }),
    new MsgExecuteContract(
      charlie.key.accAddress,
      field,
      {
        update_position: [
          {
            deposit: {
              info: {
                cw20: anchorToken,
              },
              amount: "10000000",
            },
          },
          {
            deposit: {
              info: {
                native: "uusd",
              },
              amount: "60869565",
            },
          },
          {
            bond: {
              slippage_tolerance: "0.005",
            },
          },
        ],
      },
      {
        uusd: 60869565,
      }
    ),
  ]);
  console.log(chalk.green("Done!"));
}

//--------------------------------------------------------------------------------------------------
// Test 1. Nuke
//
// current ANC-UST pool:
// uANC: 172500000 + 10000000 = 182500000
// uusd: 1050000000 + 60869565 = 1110869565
// uLP:  425587827 + 24671757 = 450259584
//
// strategy burns 170235131 + 85117565 + 24671757 = 280024453 uLP, should receive:
// uANC: 182500000 * 280024453 / 450259584 = 113499999
// uusd: 1110869565 * 280024453 / 450259584 = 690869563
//
// strategy should have also received 2 ANC from Astro generator (proxy reward)
// uANC: 113499999 + 2000000 = 115499999
//
// strategy should also have 4 ASTRO at this point, which are to be sold into UST
// ASTRO-UST pool depth: 100 ASTRO + 150 UST
// selling 4 ASTRO should return: computeXykSwapOutput(4000000, 100000000, 150000000) = 5751923 uusd
// strategy should now have uusd: 690869563 + 5751923 = 696621486
//
// alice should receive:
// uANC: 115499999 * 170235131000000 / 280024453000000 = 70215858
// uusd: 696621486 * 170235131000000 / 280024453000000 = 423496764
// uANC available: 115499999 - 70215858 = 45284141
// uusd available: 696621486 - 423496764 = 273124722
// total bond units: 280024453000000 - 170235131000000 = 109789322000000
//
// bob should receive:
// uANC: 45284141 * 85117565000000 / 109789322000000 = 35107929
// uusd: 273124722 * 85117565000000 / 109789322000000 = 211748381
// uANC available: 45284141 - 35107929 = 10176212
// uusd available: 273124722 - 211748381 = 61376341
//
// charlie should receive all remaining (10176212 uANC + 61376341 uusd)
//--------------------------------------------------------------------------------------------------

async function testNuke() {
  const aliceAncBalanceBefore = await queryCw20Balance(terra, alice.key.accAddress, anchorToken);
  const aliceUstBalanceBefore = await queryNativeBalance(terra, alice.key.accAddress, "uusd");
  const bobAncBalanceBefore = await queryCw20Balance(terra, bob.key.accAddress, anchorToken);
  const bobUstBalanceBefore = await queryNativeBalance(terra, bob.key.accAddress, "uusd");
  const charlieAncBalanceBefore = await queryCw20Balance(
    terra,
    charlie.key.accAddress,
    anchorToken
  );
  const charlieUstBalanceBefore = await queryNativeBalance(terra, charlie.key.accAddress, "uusd");

  // store code
  process.stdout.write("\nUploading nuke contract code... ");
  const codeId = await storeCode(
    deployer,
    path.join(__dirname, "../../artifacts/martian_field_closure.wasm")
  );
  console.log(chalk.green("Done!"), "Code ID:", codeId);

  // migrate contract + Nuke
  process.stdout.write("Migrating contract + nuking... ");
  const { txhash } = await sendTransaction(deployer, [
    new MsgMigrateContract(deployer.key.accAddress, field, codeId, {}),
    new MsgExecuteContract(deployer.key.accAddress, field, { refund: {} }),
    new MsgExecuteContract(deployer.key.accAddress, field, { purge_storage: {} }),
  ]);
  console.log(chalk.green("Success!"), "Txhash:", txhash);

  const aliceAncBalanceAfter = await queryCw20Balance(terra, alice.key.accAddress, anchorToken);
  const aliceUstBalanceAfter = await queryNativeBalance(terra, alice.key.accAddress, "uusd");
  const bobAncBalanceAfter = await queryCw20Balance(terra, bob.key.accAddress, anchorToken);
  const bobUstBalanceAfter = await queryNativeBalance(terra, bob.key.accAddress, "uusd");
  const charlieAncBalanceAfter = await queryCw20Balance(terra, charlie.key.accAddress, anchorToken);
  const charlieUstBalanceAfter = await queryNativeBalance(terra, charlie.key.accAddress, "uusd");

  const aliceAncBalanceChange = new BN(aliceAncBalanceAfter).sub(new BN(aliceAncBalanceBefore)).toNumber();
  const aliceUstBalanceChange = new BN(aliceUstBalanceAfter).sub(new BN(aliceUstBalanceBefore)).toNumber();
  const bobAncBalanceChange = new BN(bobAncBalanceAfter).sub(new BN(bobAncBalanceBefore)).toNumber();
  const bobUstBalanceChange = new BN(bobUstBalanceAfter).sub(new BN(bobUstBalanceBefore)).toNumber();
  const charlieAncBalanceChange = new BN(charlieAncBalanceAfter).sub(new BN(charlieAncBalanceBefore)).toNumber();
  const charlieUstBalanceChange = new BN(charlieUstBalanceAfter).sub(new BN(charlieUstBalanceBefore)).toNumber();

  expect(aliceAncBalanceChange).to.equal(70215858);
  expect(aliceUstBalanceChange).to.equal(423496764);
  expect(bobAncBalanceChange).to.equal(35107929);
  expect(bobUstBalanceChange).to.equal(211748381);
  expect(charlieAncBalanceChange).to.equal(10176212);
  expect(charlieUstBalanceChange).to.equal(61376341);
}

//--------------------------------------------------------------------------------------------------
// Main
//--------------------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nInfo"));

  console.log(`Use ${chalk.cyan(deployer.key.accAddress)} as deployer`);
  console.log(`Use ${chalk.cyan(alice.key.accAddress)} as alice`);
  console.log(`Use ${chalk.cyan(bob.key.accAddress)} as bob`);
  console.log(`Use ${chalk.cyan(charlie.key.accAddress)} as charlie`);

  console.log(chalk.yellow("\nSetup"));

  await setupTest();

  console.log(chalk.yellow("\nTests"));

  await testNuke();

  console.log(chalk.green("\nAll tests successfully completed!\n"));
})();
