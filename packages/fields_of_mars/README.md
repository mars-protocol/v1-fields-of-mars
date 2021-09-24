# Messages: Field of Mars

This crate contains message types useful for interacting with Martian Field, as well as _adapters_ which Martian Field uses to interact with other protocols.

Each adapter is a portable bundle of message types and helper functions for interacting with a specific protocol.

They are 4 adapters as of the current version:

| Adapter   | Protocol                                                                                                                                               |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Asset`   | [CW20 tokens](https://github.com/CosmWasm/cosmwasm-plus) and Terra stablecoins                                                                         |
| `RedBank` | [Mars Protocol](https://github.com/mars-protocol/protocol) liquidity pool                                                                              |
| `Staking` | [Anchor](https://github.com/Anchor-Protocol/anchor-token-contracts) or [Mirror](https://github.com/Mirror-Protocol/mirror-contracts) staking contracts |
| `Swap`    | [TerraSwap](https://github.com/terraswap/terraswap) or [Astroport](https://github.com/astroport-fi/astroport) pairs                                    |
