# Testing

These rules apply to tests and test-adjacent changes in this workspace.

## General Rules

- Prefer the smallest command that validates the change.
- Keep regression tests close to the crate that owns the behavior.
- Use integration tests in each crate's `tests/` directory when the behavior is
  cross-module or public-facing.
- Avoid live network dependencies in tests unless the test is explicitly
  marked and documented as requiring them.
- If a change spans multiple crates, split it into batches and validate each
  batch before expanding the scope.

## Recommended Validation Order

1. Run the narrowest crate-level test command first.
2. Add or update the regression test that covers the bug or contract change.
3. Run the affected crate tests again.
4. Only run the full workspace test command when the change really touches
   shared behavior.

## Notes For This Repo

- Some crates use nightly-only language features.
- Several modules are still experimental and may require manual review in
  addition to automated tests.
- If a test depends on RPC endpoints, chain SDKs, or secrets, keep the
  dependency explicit and documented.
