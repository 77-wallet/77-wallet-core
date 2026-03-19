# wallet-chain-instance

Chain-specific keypair and address wiring.

## Responsibilities

- map chain codes to the correct instance implementation
- derive keypairs from seeds and derivation paths
- generate addresses for supported chains
- provide chain-aware address and chain object enums

## Notable Entry Points

- `wallet_chain_instance::instance::ChainObject`
- `wallet_chain_instance::instance::Address`
- `wallet_chain_instance::instance::btc::BitcoinInstance`
- `wallet_chain_instance::instance::eth::EthereumInstance`
- `wallet_chain_instance::generate_address_with_xpriv`

## Notes

- This crate bridges the chain vocabulary from `wallet-types` and the core
  derivation traits from `wallet-core`.
- It is the most direct place to inspect when you need to understand how a
  chain code turns into an address generator or keypair.
