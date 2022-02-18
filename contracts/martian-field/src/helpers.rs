use cosmwasm_std::{Reply, StdError, StdResult, SubMsgExecutionResponse, Uint128};

use cw_asset::{Asset, AssetList};

/// Extract response from reply
pub fn unwrap_reply(reply: Reply) -> StdResult<SubMsgExecutionResponse> {
    reply.result.into_result().map_err(StdError::generic_err)
}

/// Assert that fund of exactly the same type and amount was sent along with a message
pub fn assert_sent_fund(expected: &Asset, received_coins: &AssetList) -> StdResult<()> {
    let received_amount = if let Some(coin) = received_coins.find(&expected.info) {
        coin.amount
    } else {
        Uint128::zero()
    };

    if received_amount != expected.amount {
        return Err(StdError::generic_err(
            format!("sent fund mismatch! expected: {}, received {}",expected, received_amount)
        ));
    } 

    Ok(())
}
