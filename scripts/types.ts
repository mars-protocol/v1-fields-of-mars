//--------------------------------------------------------------------------------------------------
// Contracts & Protocols
//--------------------------------------------------------------------------------------------------

export type Contract = {
  codeId: number;
  address: string;
};

export namespace Protocols {
  export type Astroport = {
    factory: Contract;
    pair: Contract;
    liquidityToken: Contract;
  };

  export type Mars = {
    redBank: Contract;
    oracle: Contract;
  };

  export type Anchor = {
    token: Contract;
    staking: Contract;
  };

  export type Mirror = {
    token: Contract;
    mAsset: Contract;
    staking: Contract;
  };
}

//--------------------------------------------------------------------------------------------------
// Messages
//--------------------------------------------------------------------------------------------------

// fields_of_mars::adapters::AssetInfo
export type AssetInfo = { cw20: { contract_addr: string } } | { native: { denom: string } };

// fields_of_mars::adapters::Asset
export type Asset = {
  info?: AssetInfo;
  amount: string;
};

export namespace Astroport {
  // astroport::pair::PoolResponse
  export type PoolResponse = {
    assets: Asset[];
    total_share: string;
  };
}

export namespace RedBank {
  // mars_core::red_bank::DebtResponse
  export type UserAssetDebtResponse = {
    amount: string;
    // amount is the only parameter we care about. set others to optional
    denom?: string;
    asset_label?: string;
    asset_reference?: number[];
    asset_type?: { native: {} } | { cw20: {} };
    amount_scaled?: string;
  };
}

export namespace Staking {
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
}

export namespace MartianField {
  // fields_of_mars::martian_field::Config
  export type Config = {
    red_bank: {
      contract_addr: string;
    };
    pair: {
      contract_addr: string;
      liquidity_token: string;
    };
    staking: {
      [key: string]: {
        contract_addr: string;
        asset_token: string;
        staking_token: string;
      };
    };
    [key: string]: string | object;
  };

  // fields_of_mars::martian_field::StateResponse
  export type StateResponse = {
    total_bond_units: string;
    total_debt_units: string;
  };

  // fields_of_mars::martian_field::PositionResponse
  export type PositionResponse = {
    bond_units: string;
    debt_units: string;
    unlocked_assets: Asset[];
  };

  // fields_of_mars::martian_field::HealthResponse
  export type HealthResponse = {
    bond_value: string;
    debt_value: string;
    ltv: string | null;
  };
}
