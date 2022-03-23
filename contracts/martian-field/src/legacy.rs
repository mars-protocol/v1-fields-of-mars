use cosmwasm_std::{Addr, Binary, DepsMut, Order, StdResult};
use cw_storage_plus::Map;

/// Snapshot is used by the frontend calculate user PnL. Once we build a transaction indexer that can
/// calculate PnL without relying on on-chain snapshots, this will be removed
///
/// We are only deleting snapshots here, no need to parse it into a structured data type, so we use
/// the `Binary` type here
const SNAPSHOT: Map<&Addr, Binary> = Map::new("snapshot");

/// There are 500-1000 total positions, which won't fit inside the WASM memory at the same time.
/// Therefore, we 10 keys at a time
const MAX_BATCH_SIZE: usize = 10;

/// Delete all data under the `"snapshot"` prefix
pub fn delete_snapshots(deps: DepsMut) -> StdResult<()> {
    loop {
        // Each key is a byte array, i.e. `Vec<u8>`
        let keys_raw: Vec<Vec<u8>> = SNAPSHOT
            .keys(deps.storage, None, None, Order::Ascending)
            .take(MAX_BATCH_SIZE)
            .collect();

        // Break if the number of keys is zero
        if keys_raw.is_empty() {
            break;
        }

        // Parse the keys to `Addr`
        let keys = keys_raw
            .into_iter()
            .map(|key_raw| Ok(Addr::unchecked(String::from_utf8(key_raw)?)))
            .collect::<StdResult<Vec<Addr>>>()?;

        // Delete the data of each key from storage
        keys.iter().for_each(|key| SNAPSHOT.remove(deps.storage, key));
    }

    Ok(())
}
