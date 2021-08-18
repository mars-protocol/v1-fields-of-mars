import chalk from "chalk";
import { assert } from "chai";
import { table } from "table";
import { LCDClient, LocalTerra } from "@terra-money/terra.js";

// field_of_mars::asset::Asset
export interface Asset {
  info?: object;
  amount: string;
}

// field_of_mars::martian_field::ConfigResponse
export interface Config {
  red_bank: {
    contract_addr: string;
  };
  swap: {
    pair: string;
    share_token: string;
  };
  staking: {
    [key: string]: {
      contract_addr: string;
      asset_token: string;
      staking_token: string;
    };
  };
}

// anchor_token::staking::StakerInfoResponse
export interface StakerInfoResponse {
  bond_amount: string;
}

// mirror_protocol::staking::RewardInfoResponse
export interface RewardInfoResponse {
  reward_infos: {
    asset_token?: string;
    bond_amount: string;
    pending_reward: string;
  }[];
}

// mars::red_bank::DebtResponse
export interface DebtResponse {
  debts: {
    denom?: string;
    amount: string;
  }[];
}

// astroport::pair::PoolResponse
export interface PoolResponse {
  assets: Asset[];
  total_share: string;
}

// field_of_mars::martian_field::StateResponse
export interface StateResponse {
  total_bond_units: string;
  total_debt_units: string;
}

// field_of_mars::martian_field::PositionResponse
export interface PositionResponse {
  bond_units: string;
  debt_units: string;
  unlocked_assets: Asset[];
}

// field_of_mars::martian_field::HealthResponse
export interface HealthResponse {
  bond_value: string;
  debt_value: string;
  ltv: string | null;
}

// Data to check
export interface CheckData {
  // Data related to contracts other than Marian Field, e.g. Red Bank, staking, AMM
  bond: StakerInfoResponse | RewardInfoResponse;
  debt: DebtResponse;
  pool: PoolResponse;
  // Data related to the overall state of Martian Field
  strategy: {
    state: StateResponse;
    health: HealthResponse;
  };
  // Data related to the individual users
  users: {
    address: string;
    position: PositionResponse;
    health: HealthResponse;
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
export class Checker {
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

  async check(
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
        ? ((await this.terra.wasm.contractQuery(
            this.config.staking.anchor.contract_addr,
            {
              staker_info: {
                staker: this.field,
                block_height: null,
              },
            }
          )) as StakerInfoResponse)
        : ((await this.terra.wasm.contractQuery(
            this.config.staking.mirror.contract_addr,
            {
              reward_info: {
                staker_addr: this.field,
                asset_token: undefined,
              },
            }
          )) as RewardInfoResponse);

    const debt: DebtResponse = await this.terra.wasm.contractQuery(
      this.config.red_bank.contract_addr,
      {
        debt: {
          address: this.field,
        },
      }
    );

    const pool: PoolResponse = await this.terra.wasm.contractQuery(
      this.config.swap.pair,
      {
        pool: {},
      }
    );

    // Query the global state of Martian Field
    const strategy = {
      state: (await this.terra.wasm.contractQuery(this.field, {
        state: {},
      })) as StateResponse,
      health: (await this.terra.wasm.contractQuery(this.field, {
        health: {
          user: null,
        },
      })) as HealthResponse,
    };

    // Query position and health of each user
    let users: {
      address: string;
      position: PositionResponse;
      health: HealthResponse;
    }[] = [];

    for (const user of expected.users) {
      users.push({
        address: user.address,
        position: (await this.terra.wasm.contractQuery(this.field, {
          position: {
            user: user.address,
          },
        })) as PositionResponse,
        health: (await this.terra.wasm.contractQuery(this.field, {
          health: {
            user: user.address,
          },
        })) as HealthResponse,
      });
    }

    // Combine results
    const actual: CheckData = { bond, debt, pool, strategy, users };
    // console.log(JSON.stringify(actual, null, 2));

    // Generate a comparison table
    let header = [
      chalk.magenta(test),
      "expected            ",
      "actual              ",
      "match",
    ];

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
      _generateRow(
        "debt[0].amount",
        expected.debt.debts[0].amount,
        actual.debt.debts[0].amount
      ),
      _generateRow(
        "debt[1].amount",
        expected.debt.debts[1].amount,
        actual.debt.debts[1].amount
      ),
      // pool
      _generateRow(
        "pool.assets[0]",
        expected.pool.assets[0].amount,
        actual.pool.assets[0].amount
      ),
      _generateRow(
        "pool.assets[1]",
        expected.pool.assets[1].amount,
        actual.pool.assets[1].amount
      ),
      _generateRow(
        "pool.total_share",
        expected.pool.total_share,
        actual.pool.total_share
      ),
      // state
      _generateRow(
        "state.total_bond_units",
        expected.strategy.state.total_bond_units,
        actual.strategy.state.total_bond_units
      ),
      _generateRow(
        "state.total_debt_units",
        expected.strategy.state.total_debt_units,
        actual.strategy.state.total_debt_units
      ),
      // state health
      _generateRow(
        "state.bond_value",
        expected.strategy.health.bond_value,
        actual.strategy.health.bond_value
      ),
      _generateRow(
        "state.debt_value",
        expected.strategy.health.debt_value,
        actual.strategy.health.debt_value
      ),
      _generateRow("state.ltv", expected.strategy.health.ltv, actual.strategy.health.ltv),
    ];

    for (let i = 0; i < actual.users.length; i++) {
      rows = rows.concat([
        // user 1 position
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
        _generateRow(
          `users[${i}].unlocked[0]`,
          expected.users[i].position.unlocked_assets[0].amount,
          actual.users[i].position.unlocked_assets[0].amount
        ),
        _generateRow(
          `users[${i}].unlocked[1]`,
          expected.users[i].position.unlocked_assets[1].amount,
          actual.users[i].position.unlocked_assets[1].amount
        ),
        _generateRow(
          `users[${i}].unlocked[2]`,
          expected.users[i].position.unlocked_assets[2].amount,
          actual.users[i].position.unlocked_assets[2].amount
        ),
        // user 1 health
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
        _generateRow(
          `users[${i}].ltv`,
          expected.users[i].health.ltv,
          actual.users[i].health.ltv
        ),
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
