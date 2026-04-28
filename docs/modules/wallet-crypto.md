# wallet-crypto

Keystore and cryptographic workflow helpers.

## Responsibilities

- encrypted JSON generation and decryption
- KDF configuration and selection
- keystore save and load flows
- phrase and seed wallet wrappers
- crypto utility macros and helpers

## Notable Entry Points

- `wallet_crypto::KeystoreBuilder`
- `wallet_crypto::KeystoreJsonGenerator`
- `wallet_crypto::KeystoreJsonDecryptor`
- `wallet_crypto::KdfAlgorithm`
- `wallet_crypto::sign_transaction_with_chain_code!`

## Notes

- This crate sits between the core wallet traits and the file-backed keystore
  flows.
- It is useful for both local wallet management and test fixtures.
