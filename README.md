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

## How The Pieces Fit

The workspace is layered roughly like this:

`wallet-utils` -> `wallet-types` -> `wallet-core` / `wallet-crypto` ->
`wallet-chain-instance` -> `wallet-chain-interact` -> `wallet-transport`

In practice:

- `wallet-types` carries shared chain enums, address types, and constants.
- `wallet-core` turns mnemonic material into seeds, derivation paths, and
  keypair traits.
- `wallet-crypto` handles keystore and encrypted JSON workflows.
- `wallet-chain-instance` binds a chain code plus address type to the correct
  derivation and address generator.
- `wallet-chain-interact` builds chain-specific providers, transactions, and
  signing flows.
- `wallet-transport` provides the HTTP and RPC clients those chain modules use.

## Minimal Example

This is the general shape of a derivation flow in the workspace:

```rust
use wallet_core::xpriv;
use wallet_chain_instance::instance::ChainObject;
use wallet_types::chain::network::NetworkKind;

fn derive_address() -> Result<String, Box<dyn std::error::Error>> {
    let phrase = "your twelve or twenty four word mnemonic here";
    let password = "";

    let (_root, seed) = xpriv::generate_master_key(1, phrase, password)?;
    let chain = ChainObject::new("eth", None, NetworkKind::Mainnet)?;
    let keypair = chain.gen_keypair_with_index_address_type(&seed, 0)?;

    Ok(keypair.address())
}
```

For chain-specific network work, the interaction layer follows the same
pattern: create a provider, fetch the on-chain data you need, then build or
sign the transaction with the chain-specific helper.

## Current Caveats

- The workspace uses nightly-only language features in some crates.
- Some modules still use `panic!` for invalid input and are not yet hardened
  for adversarial input.
- There is no single end-user binary yet; this is a library workspace.
- Several chain integrations depend on third-party RPC / SDK crates and may
  need environment-specific configuration.
- The APIs are not yet frozen, so treat examples as a guide to current usage
  rather than a compatibility promise.

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

If you are only checking the docs refresh, there is no extra build step beyond
reviewing the Markdown files.

## Documentation

- `docs/codex/testing.md`: how to choose and scope tests
- `docs/codex/checklists/pr-definition-of-done.md`: PR acceptance checklist
- `docs/codex/commit-message.md`: commit message format and scopes
- `docs/architecture.md`: repository layering and data flow

If you are working with Codex inside this repository, read `AGENTS.md` first so
the repo-specific boundaries stay aligned with the current workspace.
