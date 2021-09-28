export type Contract = {
  codeId: number;
  address: string;
};

export namespace Protocols {
  export type Astroport = {
    factory: Contract;
    pair: Contract;
    shareToken: Contract;
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
    staking: Contract;
  };
}

export namespace Oracle {
  export type AssetPriceResponse = {
    price: string;
    last_updated: number;
  };
}
