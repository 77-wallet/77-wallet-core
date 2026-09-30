# 77Wallet 2.2 Unified EVM Chain Support

## Active: Bound Base Additional-Fee Requests Before Local dev Merge (2026-09-30)

Scope: `wallet-chain-interact` only. Preserve the current Base native/ERC-20
transfer estimate, but reject requests with access lists or other transaction
fields not represented by the fixed unsigned-envelope allowance. Do not change
SDK, plugin, or remote branches. Add an offline regression test first.

Validation: focused `evm_additional_fee` tests, crate-level offline lib tests,
`cargo fmt --all -- --check`, and merged-result checks on local `dev`.
Stop if compilation cannot complete with available disk space or if merging
introduces a conflict or failed test; do not push.

## Active: EVM Additional Fee Core Ownership (2026-09-29)

This worktree is `codex/refactor-2.2-evm-fee-core`, based on
`9785629d190bdf6ca62e1d0a3695c5f33dba3fbd`. It is isolated from the
2.2 Core Safe preflight worktree and its uncommitted changes.

Batch scope: `wallet-chain-interact` only. Add a chain-specific additional-fee
capability for Base and Scroll using the selected `EthChain` provider; leave SDK
tracker, three-tier quote JSON, and other chains unchanged. Start with offline
RPC request/error regression tests before production implementation.

Validation: focused additional-fee tests, then
`cargo test --offline -p wallet-chain-interact --lib --no-default-features`,
`cargo check --offline -p wallet-chain-interact --lib --no-default-features`,
`cargo fmt --all -- --check`, and `git diff --check`. Use a shared `target/`
directory with reduced debug output while disk space is constrained.

Stop condition: Core tests and check pass, diff is ready for user review, and
no commit, merge, push, or SDK dependency revision change has occurred.

## Scope

- Add Polygon, Avalanche, Fantom, Arbitrum, Base, and Scroll chain codes.
- Represent Ethereum, BNB, and the six new networks through the shared
  `ChainObject::Evm` and existing `EthereumInstance` implementation.
- Keep chain IDs RPC-driven and retain the existing local EIP-1559 signing path.

## Batches

1. Extend the `wallet-types` chain contract and ETH-family membership.
2. Unify EVM address derivation and chain identity in `wallet-chain-instance`.
3. Add offline EIP-1559 construction/signing baselines in
   `wallet-chain-interact` and close exhaustive matches.

## Validation

- `cargo test --offline -p wallet-types chain::enums::chain::tests --lib`
- `cargo test --offline -p wallet-chain-instance evm_ --lib`
- `cargo test --offline -p wallet-chain-interact evm_ --lib`
- `cargo check --offline -p wallet-types -p wallet-chain-instance -p wallet-chain-interact`
- `cargo fmt --all -- --check`
- `git diff --check`

## Stop Condition

- All eight EVM-family chains use the unified EVM representation while
  preserving their canonical chain identity.
- Existing EIP-1559 signing vectors pass without live RPC calls.
- Do not add legacy transactions, `SigningBackend`, hardware protocols, SDK
  changes, backend configuration, or hard-coded production chain IDs.
