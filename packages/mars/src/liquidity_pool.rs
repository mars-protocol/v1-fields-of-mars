use cosmwasm_bignumber::{Decimal256, Uint256};
use cosmwasm_std::{HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),
    UpdateConfig {
        owner: Option<HumanAddr>,
        config: CreateOrUpdateConfig,
    },
    InitAsset {
        asset: Asset,
        asset_params: InitOrUpdateAssetParams,
    },
    UpdateAsset {
        asset: Asset,
        asset_params: InitOrUpdateAssetParams,
    },
    InitAssetTokenCallback {
        reference: Vec<u8>,
    },
    DepositNative {
        denom: String,
    },
    Borrow {
        asset: Asset,
        amount: Uint256,
    },
    RepayNative {
        denom: String,
    },
    LiquidateNative {
        collateral_asset: Asset,
        debt_asset: String,
        user_address: HumanAddr,
        receive_ma_token: bool,
    },
    FinalizeLiquidityTokenTransfer {
        sender_address: HumanAddr,
        recipient_address: HumanAddr,
        sender_previous_balance: Uint128,
        recipient_previous_balance: Uint128,
        amount: Uint128,
    },
    UpdateUncollateralizedLoanLimit {
        user_address: HumanAddr,
        asset: Asset,
        new_limit: Uint128,
    },
    UpdateUserCollateralAssetStatus {
        asset: Asset,
        enable: bool,
    },
    DistributeProtocolIncome {
        asset: Asset,
        amount: Option<Uint256>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct CreateOrUpdateConfig {
    pub treasury_contract_address: Option<HumanAddr>,
    pub insurance_fund_contract_address: Option<HumanAddr>,
    pub staking_contract_address: Option<HumanAddr>,
    pub insurance_fund_fee_share: Option<Decimal256>,
    pub treasury_fee_share: Option<Decimal256>,
    pub ma_token_code_id: Option<u64>,
    pub close_factor: Option<Decimal256>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitOrUpdateAssetParams {
    pub borrow_slope: Option<Decimal256>,
    pub loan_to_value: Option<Decimal256>,
    pub reserve_factor: Option<Decimal256>,
    pub liquidation_threshold: Option<Decimal256>,
    pub liquidation_bonus: Option<Decimal256>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Reserve {
        asset: Asset,
    },
    ReservesList {},
    Debt {
        address: HumanAddr,
    },
    UncollateralizedLoanLimit {
        user_address: HumanAddr,
        asset: Asset,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtResponse {
    pub debts: Vec<DebtInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct DebtInfo {
    pub denom: String,
    pub amount: Uint256,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    Cw20 {
        contract_addr: HumanAddr,
    },
    Native {
        denom: String,
    },
}
