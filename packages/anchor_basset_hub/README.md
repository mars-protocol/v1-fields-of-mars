# Messages: Anchor bAsset Hub

Message type definitions for the [`anchor_basset_hub`](https://github.com/Anchor-Protocol/anchor-bAsset-contracts/tree/master/contracts/anchor_basset_hub) contract.

**The code quality of `anchor_basset_hub` is very poor,** with the type definitions scattered in multiple crates and files with seemingly no logic order. This introduces tremendous problem for developers who wish to integrate the contract.

The table below summarizes the types necessary for interacting with the contract and their respective locations:

| Object                | Location                                   |
| --------------------- | ------------------------------------------ |
| `HandleMsg`           | `contracts/hub_querier/src/lib.rs`         |
| `QueryMsg`            | `contracts/anchor_basset_hub/src/msg.rs`   |
| `CurrentBatchRespnse` | `contracts/anchor_basset_hub/src/msg.rs`   |
| `Parameter`           | `contracts/anchor_basset_hub/src/state.rs` |
| `StateResponse`       | `contracts/anchor_basset_hub/src/state.rs` |

Ideally, these should all be placed in `packages/anchor_basset_contracts/src/hub.rs` and uploaded to [crates.io](https://crates.io/). Unfortunately Anchor devs didn't follow this best practice.
