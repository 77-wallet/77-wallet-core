# wallet-chain-interact

Chain-specific providers, transaction builders, and protocol logic.

## Responsibilities

- providers for chain RPC and HTTP APIs
- transaction builders and signing flows
- multisig, contract, and transfer helpers
- protocol response structs and request payloads
- per-chain operation modules

## Notable Entry Points

- `wallet_chain_interact::btc`
- `wallet_chain_interact::eth`
- `wallet_chain_interact::sol`
- `wallet_chain_interact::sui`
- `wallet_chain_interact::ton`
- `wallet_chain_interact::tron`
- `wallet_chain_interact::types`

## Notes

- This is the broadest crate in the workspace and carries most chain-specific
  behavior.
- It depends on the lower layers for shared types, derivation helpers, and
  transport wrappers.
