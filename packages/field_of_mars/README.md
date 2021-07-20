# Messages: Field of Mars

This crate contains message types useful for interacting with Martian Field contracts, as well as _adapters_ which Martian Fields use to interact with other protocols.

Each adapter is a portable bundle of message types and helper functions for interacting with a specific protocol. Each adapter also comes with a "raw" version, which can be saved in the blockchain storage and converted back to the normal version at the time of use.

They are 4 adapters as of the current version:

| Adapter   | Protocol                                                                                                                                                        |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Asset`   | [CW20 tokens](https://github.com/CosmWasm/cosmwasm-plus) and Terra stablecoins                                                                                  |
| `RedBank` | [Mars Protcol](https://github.com/mars-protocol/protocol) liquidity pool                                                                                        |
| `Staking` | [Anchor](https://github.com/Anchor-Protocol/anchor-token-contracts) or [Mirror Protocol](https://github.com/Mirror-Protocol/mirror-contracts) staking contracts |
| `Swap`    | [TerraSwap](https://github.com/terraswap/terraswap) or [Astroport](https://twitter.com/astroport_fi) pairs                                                      |
