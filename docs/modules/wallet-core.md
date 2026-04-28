# wallet-core

Core wallet traits and mnemonic / derivation helpers.

## Responsibilities

- convert mnemonic material into seeds and root keys
- define the keypair trait shared across chain instances
- define address generation and derivation traits
- provide language helpers and xpriv utilities

## Notable Entry Points

- `wallet_core::xpriv::generate_master_key`
- `wallet_core::xpriv::generate_master_key_without_check`
- `wallet_core::KeyPair`
- `wallet_core::address::GenAddress`
- `wallet_core::derive::Derive`

## Notes

- Some helpers are generic plumbing that higher-level crates depend on to keep
  chain-specific code isolated.
- Several APIs are designed for internal composition rather than direct end
  user consumption.
