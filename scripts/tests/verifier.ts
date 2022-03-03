import chalk from "chalk";
import { assert } from "chai";
import { table } from "table";
import { LCDClient, LocalTerra } from "@terra-money/terra.js";
import {
  Config,
  PoolResponse,
  UserAssetDebtResponse,
  StateResponse,
  PositionResponse,
  HealthResponse,
} from "./types";

export interface CheckData {
  bond: string;
  debt: string;
  primaryPair: PoolResponse;
  astroPair: PoolResponse;
  state: StateResponse;
  users: {
    address: string;
    position: PositionResponse;
    health: HealthResponse;
  }[];
}

function _generateRow<T = string | number | null>(name: string, expected: T, actual: T) {
  return [name, expected, actual, expected === actual ? chalk.green("true") : chalk.red("false")];
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
  config: Config;

  constructor(terra: LCDClient | LocalTerra, field: string, config: Config) {
    this.terra = terra;
    this.field = field;
    this.config = config;
  }

  async query(users: { address: string }[]): Promise<CheckData> {
    const bond: string = await this.terra.wasm.contractQuery(
      this.config.astro_generator.contract_addr,
      {
        deposit: {
          lp_token: this.config.primary_pair.liquidity_token,
          user: this.field,
        },
      }
    );

    const debt: UserAssetDebtResponse = await this.terra.wasm.contractQuery(
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

    const primaryPair: PoolResponse = await this.terra.wasm.contractQuery(
      this.config.primary_pair.contract_addr,
      {
        pool: {},
      }
    );

    const astroPair: PoolResponse = await this.terra.wasm.contractQuery(
      this.config.astro_pair.contract_addr,
      {
        pool: {},
      }
    );

    const state: StateResponse = await this.terra.wasm.contractQuery(this.field, {
      state: {},
    });

    const _users: {
      address: string;
      position: PositionResponse;
      health: HealthResponse;
    }[] = [];

    for (const user of users) {
      _users.push({
        address: user.address,
        position: await this.terra.wasm.contractQuery(this.field, {
          position: {
            user: user.address,
          },
        }),
        health: await this.terra.wasm.contractQuery(this.field, {
          health: {
            user: user.address,
          },
        }),
      });
    }

    return {
      bond,
      debt: debt.amount,
      primaryPair,
      astroPair,
      state,
      users: _users,
    };
  }

  async verify(expected: CheckData) {
    const actual = await this.query(expected.users);

    let rows: (string | number | null)[][] = [
      // header
      ["variable", "expected            ", "actual              ", "match"],
      // bond
      _generateRow("bond.amount", expected.bond, actual.bond),
      // debt
      _generateRow("debt.amount", expected.debt, actual.debt),
      // ANC-UST pool
      _generateRow(
        "primaryPair.assets[0]",
        expected.primaryPair.assets[0].amount,
        actual.primaryPair.assets[0].amount
      ),
      _generateRow(
        "primaryPair.assets[1]",
        expected.primaryPair.assets[1].amount,
        actual.primaryPair.assets[1].amount
      ),
      _generateRow(
        "primaryPair.shares",
        expected.primaryPair.total_share,
        actual.primaryPair.total_share
      ),
      // ASTRO-UST pool
      _generateRow(
        "astroPair.assets[0]",
        expected.astroPair.assets[0].amount,
        actual.astroPair.assets[0].amount
      ),
      _generateRow(
        "astroPair.assets[1]",
        expected.astroPair.assets[1].amount,
        actual.astroPair.assets[1].amount
      ),
      _generateRow(
        "astroPair.shares",
        expected.astroPair.total_share,
        actual.astroPair.total_share
      ),
    ];

    // state
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

    for (let j = 0; j < expected.state.pending_rewards.length; j++) {
      rows.push(
        _generateRow(
          `state.pending[${j}]`,
          expected.state.pending_rewards[j].amount,
          actual.state.pending_rewards[j].amount
        )
      );
    }

    // users
    for (let i = 0; i < actual.users.length; i++) {
      rows = rows.concat([
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

      for (let j = 0; j < expected.users[i].position.unlocked_assets.length; j++) {
        rows.push(
          _generateRow(
            `users[${i}].unlocked[${j}]`,
            expected.users[i].position.unlocked_assets[j].amount,
            actual.users[i].position.unlocked_assets[j].amount
          )
        );
      }

      rows = rows.concat([
        _generateRow(
          `users[${i}].bond_amount`,
          expected.users[i].health.bond_amount,
          actual.users[i].health.bond_amount
        ),
        _generateRow(
          `users[${i}].bond_value`,
          expected.users[i].health.bond_value,
          actual.users[i].health.bond_value
        ),
        _generateRow(
          `users[${i}].debt_amount`,
          expected.users[i].health.debt_amount,
          actual.users[i].health.debt_amount
        ),
        _generateRow(
          `users[${i}].debt_value`,
          expected.users[i].health.debt_value,
          actual.users[i].health.debt_value
        ),
        _generateRow(`users[${i}].ltv`, expected.users[i].health.ltv, actual.users[i].health.ltv),
      ]);
    }

    // print out the table
    process.stdout.write(
      table(rows, {
        drawHorizontalLine: (lineIndex: number, rowCount: number) => {
          return [0, 1, rowCount].includes(lineIndex);
        },
      })
    );

    // assert data match
    const match = rows.slice(1).every((row) => row[3] == chalk.green("true"));
    assert(match);
  }
}
