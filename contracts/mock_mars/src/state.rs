use cosmwasm_bignumber::Uint256;
use cosmwasm_std::{CanonicalAddr, StdResult, Storage};
use cosmwasm_storage::{bucket, bucket_read};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub static PREFIX_USERS: &[u8] = b"users";

//----------------------------------------------------------------------------------------
// STORAGE TYPES
//----------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct User {
    pub borrowed_amount: Uint256,
    pub deposited_amount: Uint256,
}

//----------------------------------------------------------------------------------------
// READ/WRITE FUNCTIONS
//----------------------------------------------------------------------------------------

pub fn read_user<S: Storage>(storage: &S, account: &CanonicalAddr) -> StdResult<User> {
    // If the user's record doesn't exist, return one with zeros
    match bucket_read(PREFIX_USERS, storage).may_load(account.as_slice()) {
        Ok(Some(user)) => Ok(user),
        Ok(None) => Ok(User {
            borrowed_amount: Uint256::zero(),
            deposited_amount: Uint256::zero(),
        }),
        Err(err) => return Err(err),
    }
}

pub fn write_user<S: Storage>(
    storage: &mut S,
    account: &CanonicalAddr,
    user: &User,
) -> StdResult<()> {
    bucket(PREFIX_USERS, storage).save(account.as_slice(), user)
}
