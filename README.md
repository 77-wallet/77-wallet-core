# 77_wallet_core

`77_wallet_core` is a Rust workspace for wallet primitives, chain-specific
address generation, transaction building, keystore handling, and transport
helpers across several chains.

This repository is still evolving. The current design has rough edges, the API
surface is not yet stable, and some modules still rely on panic-based guards or
nightly features. Treat the code as an experimental foundation rather than a
production-ready wallet SDK.

## Workspace Layout

- `wallet-utils`: shared helpers for logging, parsing, signing, encoding,
  system information, and general-purpose utilities.
- `wallet-types`: shared chain enums, constants, errors, and value objects.
- `wallet-core`: common wallet traits for keypairs, derivation, language
  helpers, and extended private key support.
- `wallet-crypto`: mnemonic, seed, keystore, KDF, and encrypted JSON helpers.
- `wallet-transport`: HTTP / MQTT / RPC transport wrappers and transport
  errors.
- `wallet-chain-instance`: chain-specific instance types for deriving keys and
  generating addresses.
- `wallet-chain-interact`: chain interaction logic, providers, protocol types,
  transaction builders, signatures, multisig flows, and token operations.

## Supported Chains

The workspace currently contains code for:

- Bitcoin
- Litecoin
- Dogecoin
- Ethereum
- BNB Smart Chain
- Solana
- Sui
- Tron
- Ton

## What This Repository Provides

- BIP32 / BIP39 derivation helpers
- Keystore encryption and decryption flows
- Chain-aware address generation
- Chain-specific transaction builders and signing helpers
- Transport abstractions for HTTP and RPC-style communication
- Shared constants and typed responses for common wallet operations

## Current Caveats

- The workspace uses nightly-only language features in some crates.
- Some modules still use `panic!` for invalid input and are not yet hardened
  for adversarial input.
- There is no single end-user binary yet; this is a library workspace.
- Several chain integrations depend on third-party RPC / SDK crates and may
  need environment-specific configuration.

## Getting Started

This workspace is intended to be used from a recent Rust nightly toolchain.

```bash
rustup toolchain install nightly
cargo +nightly test
```

To focus on one crate:

```bash
cargo +nightly test -p wallet-utils
cargo +nightly test -p wallet-chain-interact
```

## Documentation

- `docs/codex/testing.md`: how to choose and scope tests
- `docs/codex/checklists/pr-definition-of-done.md`: PR acceptance checklist
- `docs/codex/commit-message.md`: commit message format and scopes

If you are working with Codex inside this repository, read `AGENTS.md` first so
the repo-specific boundaries stay aligned with the current workspace.
