import chalk from "chalk";
import { expect } from "chai";
import {
  LocalTerra,
  MsgExecuteContract,
  MsgMigrateContract,
} from "@terra-money/terra.js";
import { deployBLuna } from "./fixture";
import { queryTokenBalance, queryNativeTokenBalance, sendTransaction } from "./helpers";

const terra = new LocalTerra();
const deployer = terra.wallets.test1;
const user1 = terra.wallets.test2;
const user2 = terra.wallets.test3;

let bLunaHub: string;
let bLunaToken: string;

//----------------------------------------------------------------------------------------
// TEST STATE
//----------------------------------------------------------------------------------------

async function testState() {
  process.stdout.write("Should return correct state info... ");

  const response = await terra.wasm.contractQuery(bLunaHub, {
    state: {},
  });
  expect(response).to.deep.equal({
    exchange_rate: "1.000007185261045766",
    total_bond_amount: "0",
    last_index_modification: 0,
    prev_hub_balance: "0",
    actual_unbonded_amount: "0",
    last_unbonded_time: 0,
    last_processed_batch: 0,
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST PARAMETERS
//----------------------------------------------------------------------------------------

async function testParameters() {
  process.stdout.write("Should return correct parameters... ");

  const response = await terra.wasm.contractQuery(bLunaHub, {
    parameters: {},
  });
  expect(response).to.deep.equal({
    er_threshold: "1",
    peg_recovery_fee: "0.005",
    epoch_period: 0,
    unbonding_period: 0,
    underlying_coin_denom: "ngmi",
    reward_denom: "hfsp",
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST CURRENT BATCH
//----------------------------------------------------------------------------------------

async function testCurrentBatch() {
  process.stdout.write("Should return correct current batch info... ");

  const response = await terra.wasm.contractQuery(bLunaHub, {
    current_batch: {},
  });
  expect(response).to.deep.equal({
    id: 0,
    requested_with_fee: "12345",
  });

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST BOND 1
//----------------------------------------------------------------------------------------

async function testBond1() {
  process.stdout.write("Should bond Luna when bLUNA:LUNA is on-peg... ");

  await sendTransaction(terra, user1, [
    new MsgExecuteContract(
      user1.key.accAddress,
      bLunaHub,
      {
        bond: {
          validator: user1.key.accAddress, // doesn't matter for this mock contract
        },
      },
      {
        uluna: "69000000", // 69 LUNA
      }
    ),
  ]);

  // bLUNA:LUNA ratio is on peg (exchange_rate >= er_threshold), no peg fee is charged
  // mint_amount = payment.amount / exchange_rate
  // = 69000000 / 1.000007185261045766
  // = 68999504 ubluna
  const user1BLunaBalance = await queryTokenBalance(
    terra,
    user1.key.accAddress,
    bLunaToken
  );
  expect(user1BLunaBalance).to.equal("68999504");

  // bLuna Hub should have received 69 LUNA (Note: transfer of LUNA is NOT subject to tax)
  const hubLunaBalance = await queryNativeTokenBalance(terra, bLunaHub, "uluna");
  expect(hubLunaBalance).to.equal("69000000");

  // State should have been updated
  const response = (await terra.wasm.contractQuery(bLunaHub, {
    state: {},
  })) as { total_bond_amount: string };
  expect(response.total_bond_amount).to.equal("69000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST MIGRATE
//----------------------------------------------------------------------------------------

async function testMigrate() {
  process.stdout.write("Should migrate... ");

  // We don't actually change the code ID during migration
  const codeId = (await terra.wasm.contractInfo(bLunaHub)).code_id;

  await sendTransaction(terra, deployer, [
    new MsgMigrateContract(deployer.key.accAddress, bLunaHub, codeId, {
      new_exchange_rate: "0.985",
    }),
  ]);

  // Exchange rate stored in State should have been updated
  const stateResponse = (await terra.wasm.contractQuery(bLunaHub, {
    state: {},
  })) as { exchange_rate: string };
  expect(stateResponse.exchange_rate).to.equal("0.985");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// TEST BOND 2
//----------------------------------------------------------------------------------------

async function testBond2() {
  process.stdout.write("Should bond Luna when bLUNA:LUNA is off-peg... ");

  await sendTransaction(terra, user2, [
    new MsgExecuteContract(
      user2.key.accAddress,
      bLunaHub,
      {
        bond: {
          validator: user2.key.accAddress, // doesn't matter for this mock contract
        },
      },
      {
        uluna: "420000000", // 420 LUNA
      }
    ),
  ]);

  // bLUNA:LUNA ratio is off peg (exchange_rate < er_threshold). Peg fee is to be charged
  // mint_amount = payment.amount / exchange_rate
  // = 420000000 / 0.985
  // = 426395939
  // max_peg_fee = mint_amount * peg_recovery_fee
  // = 426395939 * 0.005
  // = 2131979
  // required_peg_fee
  // = (total_supply + mint_amount + requested_with_fee) - (total_bond_amount + payment.amount)
  // = (68999504 + 426395939 + 12345) - (69000000 + 420000000)
  // = 6407788
  // peg_fee = min(max_peg_fee, required_peg_fee)
  // = min(2131979, 6407788)
  // = 2131979
  // mint_amount_with_fee = mint_amount - peg_fee
  // = 426395939 - 2131979
  // = 424263960
  const user2BLunaBalance = await queryTokenBalance(
    terra,
    user2.key.accAddress,
    bLunaToken
  );
  expect(user2BLunaBalance).to.equal("424263960");

  // bLuna Hub should have received 69 + 420 = 489 LUNA
  const hubLunaBalance = await queryNativeTokenBalance(terra, bLunaHub, "uluna");
  expect(hubLunaBalance).to.equal("489000000");

  // State should have been updated
  const response = (await terra.wasm.contractQuery(bLunaHub, {
    state: {},
  })) as { total_bond_amount: string };
  expect(response.total_bond_amount).to.equal("489000000");

  console.log(chalk.green("Passed!"));
}

//----------------------------------------------------------------------------------------
// MAIN
//----------------------------------------------------------------------------------------

(async () => {
  console.log(chalk.yellow("\nTest: Setup"));

  ({ bLunaHub, bLunaToken } = await deployBLuna(terra, deployer));

  console.log(chalk.yellow("\nTest: Mock bLuna"));

  await testState();
  await testParameters();
  await testCurrentBatch();
  await testBond1();
  await testMigrate();
  await testBond2();

  console.log("");
})();
