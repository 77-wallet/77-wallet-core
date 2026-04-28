# wallet-utils

Shared helpers used across the workspace.

## Responsibilities

- logging and tracing setup
- parsing, conversion, and formatting helpers
- cryptographic utility wrappers
- address, unit, and sign helpers
- file, time, ping, and system utilities

## Notable Entry Points

- `wallet_utils::init_log`
- `wallet_utils::init_test_log`
- `wallet_utils::here!`
- `wallet_utils::address`
- `wallet_utils::sign`

## Notes

- This crate is intentionally broad because many higher-level crates depend on
  it.
- Some helpers are low-level building blocks that other modules compose into
  wallet flows.
