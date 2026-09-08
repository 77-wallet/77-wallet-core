# Pre-merge correction for f219b17

Base: `gitlab/codex/feat-2.2-evm-chains-core` (`f219b17`).

## Constraints

Preserve `ChainObject::Eth`, `ChainObject::Bnb`, and `Address::BnbAddress`.
Reuse EthereumInstance for the six canonical new chain codes; reject aliases.
Keep production chain IDs supplied by RPC. No SDK, backend, legacy transaction,
SigningBackend, or hardware protocol changes.

## Sequential batches

1. **chain-instance compatibility:** restore original public variants; route new
   chains through Eth with their own ChainCode. Add downstream-shaped regression
   coverage and inspect current wallet_sdk call sites.
   Validate: `cargo test -p wallet-chain-instance evm_ --lib` and
   `cargo test -p wallet-chain-instance --test evm_public_compat` and
   `cargo check -p wallet-chain-instance`. Stop before other crate edits until passed.
2. **chain-interact signing:** review provider/chain changes, retain only private
   signing extraction needed for fixed offline EIP-1559 vectors; preserve existing
   broadcast behavior. Validate: `cargo test -p wallet-chain-interact evm_ --lib`
   and `cargo check -p wallet-chain-interact`. Stop when vectors pass and production
   chain IDs are confirmed RPC-driven.
3. **wallet-core cleanup:** restore unrelated language lines exactly from parent.
   Validate through final dependent crate checks.
4. **Final acceptance:** `cargo test -p wallet-types chain::enums::chain::tests --lib`,
   `cargo check -p wallet-types -p wallet-chain-instance -p wallet-chain-interact`,
   `cargo fmt --all -- --check`, and `git diff --check`. Review DoD, commit using
   Conventional Commits, push HEAD to `codex/feat-2.2-evm-chains-core` without force.

## Results

- Batch 1 passed offline: 2 EVM unit tests, 1 public compatibility integration
  test, and cargo check for wallet-chain-instance. Original enum variant sets and
  payload types restored exactly. Six new chains use Eth with distinct ChainCode.
- SDK source inspected at `/Users/apple/Work/rust/77_wallet_sdk`, HEAD `85c9c91f`.
  No direct Eth/Bnb/BnbAddress variant references exist in this checkout; generic
  construction/derivation calls remain unchanged. Integration test compiles the
  original public variant construction and exhaustive ChainObject match shape.
  This is focused API verification, not a full SDK build.
- Batch 2 passed offline: 3 EVM tests and cargo check. chain.rs has test-only
  additions; private sign_transaction
  extraction remains on build_signed_raw_transaction for offline vector coverage.
  Restored send_raw_transaction exactly to preserve its existing error behavior.
- language.rs restoration conflicts with rustfmt (the deleted lines are trailing
  blank lines). User preference requested while independent work continues.

- Batch 3: restored language.rs byte-for-byte from f219b17 parent, following the
  explicit restoration request. Trailing blank lines conflict with rustfmt and
  git diff whitespace checks; preference question remains unanswered.
- Final documentation: document canonical EVM codes, compatible instance variants,
  and RPC chain ID behavior in README (DoD public behavior documentation).

## Final validation and DoD

All cargo test/check commands used `--offline` after the initial online attempt
failed DNS resolution; cached dependencies resolved successfully. Cargo.lock is
ignored by the repository and is not committed. Build artifacts remain in target/.

- `cargo test --offline -p wallet-chain-instance evm_ --lib`: 2 passed.
- `cargo test --offline -p wallet-chain-instance --test evm_public_compat`: 1 passed.
- `cargo test --offline -p wallet-chain-instance test_parse_eth_path --lib`: 1 passed.
- `cargo test --offline -p wallet-chain-interact evm_ --lib`: 3 passed.
- `cargo test --offline -p wallet-types chain::enums::chain::tests --lib`: 4 passed.
- Per-batch cargo checks and final
  `cargo check --offline -p wallet-types -p wallet-chain-instance -p wallet-chain-interact`:
  passed. Existing deprecation/future-incompatibility warnings remain.
- `cargo fmt --all -- --check`: fails only on the explicitly restored two trailing
  blank lines in wallet-core/src/language.rs.
- `git diff --check`: fails only on the same restored EOF blank lines.
- `git diff --check f219b17^`: passes for the complete feature diff, because
  language.rs is identical to the pre-feature base.
- Programmatic comparison confirms original Address/ChainObject enum declarations,
  original send_raw_transaction implementation, and provider public method set.
- DoD: scope/boundaries, focused tests, README behavior documentation, and no added
  secrets verified. Formatting exceptions above remain unresolved; this is not an
  all-green formatting report. No live RPC or full SDK build was performed.
