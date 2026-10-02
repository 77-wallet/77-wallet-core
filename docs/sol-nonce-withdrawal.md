# Solana nonce withdrawals

`SolanaChain::account_type(address)` returns a `Result<SolAccountType>`.
Account absence is `NotFound`; valid non-nonce accounts are `Other`.
Missing nonce data is `ParseError`, and Base64/bincode errors retain their
existing `UtilsError` type. Decode failures are never classified as `Other`.

Construct `NonceWithdrawOpt` with the nonce address, its on-chain authority,
destination and an exact SOL amount string. Default fee payer is the authority.
An optional nonce private key selects the nonce account as fee payer.

```rust,ignore
let mut params = NonceWithdrawOpt::new(nonce_address, authority, to, value)?;
params.prepare(chain.get_provider(), &authority_key, nonce_key.as_ref()).await?;
let instructions = params.instructions().await?;
let payer_key = nonce_key.unwrap_or(authority_key);
let hash = chain.exec_transaction(params, payer_key, fee, instructions, 0).await?;
```

`prepare` checks nonce state, authority, and both supplied keys.
Signer configuration changes only after all checks succeed. It does not query
fees or rent, or perform balance checks. The caller can use existing balance,
estimate_fee_v1, and minimum-rent APIs for business prechecks.
Do not change parameters after preparation. `exec_transaction` signs and broadcasts using its existing
`other_keypair` mechanism; `withdraw_nonce_sol` is no longer provided.

When the nonce pays, the exact withdrawal amount must leave rent-exempt funds
or equal the balance after deducting the actual message fee to close it.
RPC preflight remains authoritative for state changes and closure restrictions.
