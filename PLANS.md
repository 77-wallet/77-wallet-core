# 77Wallet 2.2 Unified EVM Chain Support

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
