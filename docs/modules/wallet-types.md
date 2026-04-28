# wallet-types

Shared domain types and constants for the wallet workspace.

## Responsibilities

- chain codes and chain types
- address types and address categories
- network kind definitions
- shared constants for derivation paths, decimals, and token addresses
- typed error and value object definitions

## Notable Entry Points

- `wallet_types::chain::chain::ChainCode`
- `wallet_types::chain::network::NetworkKind`
- `wallet_types::chain::address::r#type::AddressType`
- `wallet_types::constant`
- `wallet_types::valueobject`

## Notes

- This crate is the shared vocabulary for the rest of the workspace.
- It intentionally stays lightweight so other crates can depend on it without
  pulling in chain-specific behavior.
