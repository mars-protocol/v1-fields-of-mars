import chalk from "chalk";
import { assert } from "chai";
import { table } from "table";
import { LCDClient, LocalTerra } from "@terra-money/terra.js";
import { Astroport, RedBank, Staking, MartianField } from "./types";

// Data to check
export interface CheckData {
  bond: Staking.StakerInfoResponse | Staking.RewardInfoResponse;
  debt: RedBank.UserAssetDebtResponse;
  pool: Astroport.PoolResponse;
  state: MartianField.StateResponse;
  users: {
    address: string;
    position: MartianField.PositionResponse;
    health: MartianField.HealthResponse;
  }[];
}

function _generateRow(
  name: string,
  expectedValue: string | null | undefined,
  actualValue: string | null | undefined
) {
  return [
    name,
    expectedValue,
    actualValue,
    expectedValue == actualValue ? chalk.green("true") : chalk.red("false"),
  ];
}

/**
 * @notice Helper for checking whether contract state matches expected values
 */
export class Verifier {
  // Terra instance
  terra: LCDClient | LocalTerra;
  // Address of Martian Field contract
  field: string;
  // Config of Martian Field contract
  config: MartianField.Config;

  constructor(terra: LCDClient | LocalTerra, field: string, config: MartianField.Config) {
    this.terra = terra;
    this.field = field;
    this.config = config;
  }

  async verify(
    // Hash of the transaction
    txhash: string,
    // Name of the test
    test: string,
    // Expected contract state
    expected: CheckData
  ) {
    // Query external contracts
    const bond =
      "anchor" in this.config.staking
        ? ((await this.terra.wasm.contractQuery(this.config.staking.anchor.contract_addr, {
            staker_info: {
              staker: this.field,
              block_height: null,
            },
          })) as Staking.StakerInfoResponse)
        : ((await this.terra.wasm.contractQuery(this.config.staking.mirror.contract_addr, {
            reward_info: {
              staker_addr: this.field,
              asset_token: undefined,
            },
          })) as Staking.RewardInfoResponse);

    const debt: RedBank.UserAssetDebtResponse = await this.terra.wasm.contractQuery(
      this.config.red_bank.contract_addr,
      {
        user_asset_debt: {
          user_address: this.field,
          asset: {
            native: {
              denom: "uusd",
            },
          },
        },
      }
    );

    const pool: Astroport.PoolResponse = await this.terra.wasm.contractQuery(
      this.config.pair.contract_addr,
      {
        pool: {},
      }
    );

    // Query the global state of Martian Field
    const state: MartianField.StateResponse = await this.terra.wasm.contractQuery(this.field, {
      state: {},
    });

    // Query position and health of each user
    let users: {
      address: string;
      position: MartianField.PositionResponse;
      health: MartianField.HealthResponse;
    }[] = [];

    for (const user of expected.users) {
      users.push({
        address: user.address,
        position: (await this.terra.wasm.contractQuery(this.field, {
          position: {
            user: user.address,
          },
        })) as MartianField.PositionResponse,
        health: (await this.terra.wasm.contractQuery(this.field, {
          health: {
            user: user.address,
          },
        })) as MartianField.HealthResponse,
      });
    }

    // Combine results
    const actual: CheckData = { bond, debt, pool, state, users };
    // console.log(JSON.stringify(actual, null, 2));

    // Generate a comparison table
    let header = [chalk.magenta(test), "expected            ", "actual              ", "match"];

    let rows = [
      header,
      // bond
      _generateRow(
        "bond.amount",
        "bond_amount" in expected.bond
          ? expected.bond.bond_amount
          : expected.bond.reward_infos.length > 0
          ? expected.bond.reward_infos[0].bond_amount
          : "0",
        "bond_amount" in actual.bond
          ? actual.bond.bond_amount
          : actual.bond.reward_infos.length > 0
          ? actual.bond.reward_infos[0].bond_amount
          : "0"
      ),
      // debt
      _generateRow("debt.amount", expected.debt.amount, actual.debt.amount),
      // pool
      _generateRow("pool.assets[0]", expected.pool.assets[0].amount, actual.pool.assets[0].amount),
      _generateRow("pool.assets[1]", expected.pool.assets[1].amount, actual.pool.assets[1].amount),
      _generateRow("pool.total_share", expected.pool.total_share, actual.pool.total_share),
    ];

    // state units
    rows = rows.concat([
      _generateRow(
        "state.total_bond_units",
        expected.state.total_bond_units,
        actual.state.total_bond_units
      ),
      _generateRow(
        "state.total_debt_units",
        expected.state.total_debt_units,
        actual.state.total_debt_units
      ),
    ]);

    // state pending rewards
    // user unlocked assets
    for (let j = 0; j < expected.state.pending_rewards.length; j++) {
      rows.push(
        _generateRow(
          `state.pending[${j}]`,
          expected.state.pending_rewards[j].amount,
          actual.state.pending_rewards[j].amount
        )
      );
    }

    for (let i = 0; i < actual.users.length; i++) {
      rows = rows.concat([
        // user position
        _generateRow(
          `users[${i}].bond_units`,
          expected.users[i].position.bond_units,
          actual.users[i].position.bond_units
        ),
        _generateRow(
          `users[${i}].debt_units`,
          expected.users[i].position.debt_units,
          actual.users[i].position.debt_units
        ),
      ]);

      // user unlocked assets
      for (let j = 0; j < expected.users[i].position.unlocked_assets.length; j++) {
        rows.push(
          _generateRow(
            `users[${i}].unlocked[${j}]`,
            expected.users[i].position.unlocked_assets[j].amount,
            actual.users[i].position.unlocked_assets[j].amount
          )
        );
      }

      // user health
      rows = rows.concat([
        _generateRow(
          `users[${i}].bond_value`,
          expected.users[i].health.bond_value,
          actual.users[i].health.bond_value
        ),
        _generateRow(
          `users[${i}].debt_value`,
          expected.users[i].health.debt_value,
          actual.users[i].health.debt_value
        ),
        _generateRow(`users[${i}].ltv`, expected.users[i].health.ltv, actual.users[i].health.ltv),
      ]);
    }

    // Print the comparison table
    process.stdout.write(
      table(rows, {
        header: {
          content: `${chalk.cyan("txhash:")} ${txhash}`,
          alignment: "left",
        },
        drawHorizontalLine: (lineIndex: number, rowCount: number) => {
          return [0, 1, 2, rowCount].includes(lineIndex);
        },
      })
    );

    // Assert actual data match expected ones
    const match = rows.slice(1).every((row) => row[3] == chalk.green("true"));
    assert(match);
  }
}
