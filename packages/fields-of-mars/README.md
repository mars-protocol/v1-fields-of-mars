# Fields of Mars: Common Types

This crate contains common types used in Martian Field, as well as **adapters** used to communicate with other contracts.

## Adapters

Adapters provide a modular way to execute or query other contracts. This crates includes the following adapters:

| adapter               | description                         |
| --------------------- | ----------------------------------- |
| `adapters::Generator` | Astroport LP token staking contract |
| `adapters::Oracle`    | Mars Protocol oracle                |
| `adapters::Pair`      | Astroport pairs                     |
| `adapters::RedBank`   | Mars Protocol lending market        |

## License

Contents of this crate are open source under [GNU General Public License v3](https://www.gnu.org/licenses/gpl-3.0.en.html) or later.
