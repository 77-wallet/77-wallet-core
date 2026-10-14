# 77Wallet 2.2 Unified EVM Chain Support

## Remove blacklist gas-price round trip (2026-10-13)

- Scope: `wallet-chain-interact` Sui blacklist request only. Omit gas payment
  in the structured, checks-disabled simulation and let the node resolve its
  gas defaults. Remove the provider's extra reference-gas-price RPC.
- Preserve existing user edits and normal fee-estimation requests.
- Validation: focused offline provider tests, crate library check, changed-file
  rustfmt check and diff check. Stop with changes uncommitted; no live RPC.
- Evidence: six focused offline tests passed, including gas-payment omission
  and preservation of explicit prices for fee simulations. Crate library check,
  changed-file rustfmt check and diff check passed; live RPC was not exercised.

## Sui blacklist rebase resolution (2026-10-13)

- Scope: one batch in `wallet-chain-interact`, Sui provider only. Preserve the
  gRPC migration and port dev's `coin_deny_status` / `black_address` using
  `GetObject` owner metadata and `SimulateTransaction` command outputs.
- Preserve current/next epoch semantics. Missing or malformed responses and
  simulation failures must return errors, never an unblocked default.
- Validation: offline request/response regression tests under
  `sui::provider::tests`, `cargo check --offline -p wallet-chain-interact --lib`,
  changed-file rustfmt check, and `git diff --check`.
- Stop condition: conflict resolved and focused validation passes; continue
  the pending local rebase without pushing. If dependencies prevent validation,
  record the blocker before continuing the rebase.
- Evidence: all five offline provider regression tests passed;
  `cargo check --offline -p wallet-chain-interact --lib --tests`, changed-file
  rustfmt check and `git diff --check` passed. Live RPC was not exercised.

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

## Safe deployment preflight follow-up (2026-09-29)

- Batch 1: in `wallet-chain-interact` only, expose an `EthChain` Safe deployment
  preflight for Polygon. Check chain ID 137 and nonempty valid code at the
  existing canonical Safe factory and singleton addresses. Keep ETH/BNB
  behavior unchanged. Add offline fake-RPC success, wrong-chain, missing-code,
  and malformed-code tests. Validate focused crate tests, crate check,
  formatting, and diff check. Stop before SDK integration and other EVM chains.
- Batch 2: SDK calls the Core preflight before charging a deployment service
  fee and removes duplicated Safe addresses and RPC validation from SDK.
  Validate focused wallet-api tests and library check. Stop before transaction
  execution or enabling Avalanche, Arbitrum, Base, and Scroll.

## Avalanche Safe preflight (2026-09-29)

- Batch 1, Core only: allow Avalanche mainnet chain ID 43114 through the
  existing Safe v1.3.0 factory/singleton preflight. Keep testnet and other
  unsupported chains closed. Use the existing offline fake RPC to cover
  valid bytecode, wrong chain ID, and missing contract code. Validate focused
  `wallet-chain-interact` tests, crate check, formatting, and diff check.
  Stop before SDK routing, fee handling, or live transaction execution.
- Batch 2, SDK only: enable Avalanche in the multisig adapter after the Core
  commit is available; add business-entry tests for RPC rejection before
  service fee. Stop before transfer/sign/execute and the other new chains.

Evidence: Safe's v1.3.0 `gnosis_safe_l2.json` and `proxy_factory.json` both
list chain ID 43114 with canonical deployments. The focused offline Core
preflight suite passed 9 tests; the crate `--lib --tests` check, changed-file
format check, and diff check passed. No live Avalanche RPC was queried.

## Arbitrum One Safe preflight (2026-09-29)

- Core batch: in `wallet-chain-interact` only, allow Arbitrum One mainnet chain
  ID 42161 through the existing Safe v1.3.0 factory/singleton preflight.
  Preserve testnet and unsupported-chain rejection. Add offline fake-RPC tests
  for valid contracts, wrong chain ID, missing singleton, and unsupported
  testnet. Validate focused `safe_preflight` tests, crate check, formatting,
  and diff check. Stop before SDK routing, fees, or transaction execution;
  leave changes uncommitted and unpushed for review.
- Source: Arbitrum's bridge docs list Arbitrum One as 42161. Safe's official
  v1.3.0 `proxy_factory.json` and `gnosis_safe_l2.json` list chain 42161 with
  the canonical addresses already used by this module.
- Evidence: the focused offline `safe_preflight` suite passed 13 tests;
  `cargo check -p wallet-chain-interact --lib --tests --offline` and
  `git diff --check` passed. No live Arbitrum RPC was queried.

## Base Safe preflight (2026-09-29)

- Core batch: in `wallet-chain-interact` only, allow Base Mainnet chain ID
  8453 through the existing Safe v1.3.0 factory/singleton preflight.
  Keep Base Sepolia and unsupported chains closed. Cover matching RPC and
  contract bytecode, wrong chain ID, missing singleton, and unsupported
  testnet with the offline fake RPC. Validate focused `safe_preflight` tests,
  crate check, formatting, and diff check. Stop before SDK routing, fee
  handling, transaction execution, or any commit/push.
- Source: Base's `eth_chainId` documentation lists mainnet 8453 (`0x2105`),
  and Safe's official v1.3.0 factory and L2 singleton registries list chain
  8453 with the canonical addresses already used by this module.
- Evidence: the focused offline `safe_preflight` suite passed 17 tests;
  `cargo check -p wallet-chain-interact --lib --tests --offline`, formatting,
  and `git diff --check` passed. No live Base RPC was queried.

## Scroll Safe preflight (2026-09-29)

- Core batch: in `wallet-chain-interact` only, allow Scroll Mainnet chain ID
  534352 through the existing Safe v1.3.0 factory/singleton preflight.
  Keep unsupported networks closed. Cover matching RPC and bytecode, wrong
  chain ID, missing singleton, and unsupported testnet with the offline fake
  RPC. Validate focused `safe_preflight` tests, crate check, formatting, and
  diff check. Stop before SDK routing, fee handling, transaction execution,
  or any commit/push.
- Source: Scroll's developer docs list mainnet chain ID 534352 (`0x82750`).
  Safe's official v1.3.0 factory and L2 singleton registries list this chain
  with the canonical addresses already used by the module.
- Evidence: the focused offline `safe_preflight` suite passed 21 tests;
  `cargo check -p wallet-chain-interact --lib --tests --offline`, formatting,
  and `git diff --check` passed. No live Scroll RPC was queried.


## Sui recharge scanner compatibility (2026-10-13)

- Batch 1: only the Sui Provider read flow in `wallet-chain-interact`. Extend
  checkpoint results with transaction digests and queried transactions with gas
  owner and typed balance changes. Update read masks and validate required fields
  at the gRPC mapping boundary. Add offline request/mapping/batch regressions.
  Validate `cargo test -p wallet-chain-interact --lib sui::provider::tests --offline`
  and focused formatting/diff checks. Stop before editing the plugin until this
  batch passes; preserve the existing uncommitted deny-list changes.
- Batch 2: only `plugin-sui` in the recharge plugins workspace. Switch scans and
  transaction details back to the enriched Provider DTOs; remove the temporary
  reader and its direct gRPC dependencies. Preserve transfer/fee behavior and
  existing config edits. Validate `cargo check -p plugin-sui --offline` and
  `cargo test -p plugin-sui --lib --offline`. Stop after offline verification;
  no live RPC, transaction submission, commit, or push.
