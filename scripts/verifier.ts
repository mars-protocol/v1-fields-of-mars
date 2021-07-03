import { expect } from "chai";
import { LocalTerra, Wallet } from "@terra-money/terra.js";

/**
 * @notice A bundle of helper functions for verifying the state of Field of Mars strategy
 */
export class Verifier {
  terra: LocalTerra;
  assetToken: string;
  mars: string;
  staking: string;
  strategy: string;

  constructor(terra: LocalTerra, contracts: { [key: string]: string }) {
    this.terra = terra;
    this.assetToken = contracts.assetToken;
    this.mars = contracts.mars;
    this.staking = contracts.staking;
    this.strategy = contracts.strategy;
  }

  /**
   * @notice Verify whether the strategy's config equals the expected value
   */
  async verifyConfig(expectedResponse: object) {
    const response = await this.terra.wasm.contractQuery(this.strategy, {
      config: {},
    });
    expect(response).to.deep.equal(expectedResponse);
  }

  /**
   * @notice Verify whether the strategy's state equals the expected value
   */
  async verifyState(expectedResponse: object) {
    const response = await this.terra.wasm.contractQuery(this.strategy, {
      state: {},
    });
    expect(response).to.deep.equal(expectedResponse);
  }

  /**
   * @notice Verify whether the a user's position equals the expected value
   */
  async verifyPosition(user: Wallet, expectResponse: object) {
    const response = await this.terra.wasm.contractQuery(this.strategy, {
      position: {
        user: user.key.accAddress,
      },
    });
    expect(response).to.deep.equal(expectResponse);
  }

  /**
   * @notice Verify whether the strategy's debt owed to Mars equals the expected value
   */
  async verifyDebt(amount: string) {
    const response = await this.terra.wasm.contractQuery(this.mars, {
      debt: {
        address: this.strategy,
      },
    });
    expect(response).to.deep.equal({
      debts: [
        {
          denom: "uusd",
          amount: amount,
        },
      ],
    });
  }

  /**
   * @notice Verify whether the strategy's bonded asset equals the expected value
   */
  async verifyBondInfo(stakingType: "anchor" | "mirror", bondAmount: string) {
    if (stakingType == "anchor") {
      await this._verifyAnchorBondInfo(bondAmount);
    } else {
      await this._verifyMirrorBondInfo(bondAmount);
    }
  }

  async _verifyAnchorBondInfo(bondAmount: string) {
    const response = await this.terra.wasm.contractQuery(this.staking, {
      staker_info: {
        staker: this.strategy,
        block_height: undefined,
      },
    });
    expect(response).to.deep.equal({
      staker: this.strategy,
      reward_index: "0",
      bond_amount: bondAmount,
      pending_reward: "1000000",
    });
  }

  async _verifyMirrorBondInfo(bondAmount: string) {
    const response = await this.terra.wasm.contractQuery(this.staking, {
      reward_info: {
        staker_addr: this.strategy,
        asset_token: this.assetToken,
      },
    });
    expect(response).to.deep.equal({
      staker_addr: this.strategy,
      reward_infos: [
        {
          asset_token: this.assetToken,
          bond_amount: bondAmount,
          pending_reward: "1000000",
          is_short: false,
        },
      ],
    });
  }
}
