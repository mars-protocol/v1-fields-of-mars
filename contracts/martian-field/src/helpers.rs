use cosmwasm_std::{Coin, Reply, StdError, StdResult, SubMsgExecutionResponse, Uint128};

use cw_asset::{Asset, AssetList};

/// Extract response from reply
pub fn unwrap_reply(reply: Reply) -> StdResult<SubMsgExecutionResponse> {
    reply.result.into_result().map_err(StdError::generic_err)
}

/// Assert that fund of exactly the same type and amount was sent along with a message
pub fn assert_sent_fund(expected: &Asset, received: &[Coin]) -> StdResult<()> {
    let list = AssetList::from(received);

    let received_amount = if let Some(received_asset) = list.find(&expected.info) {
        received_asset.amount
    } else {
        Uint128::zero()
    };

    if received_amount == expected.amount {
        Ok(())
    } else {
        Err(StdError::generic_err(format!(
            "sent fund mismatch! expected: {}, received {}",
            expected, received_amount
        )))
    }
}
