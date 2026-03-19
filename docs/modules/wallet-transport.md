# wallet-transport

Transport wrappers for HTTP, RPC, and request construction.

## Responsibilities

- HTTP client wrapper
- RPC client wrapper
- request builder helpers
- transport error types
- request and response structs used by chain providers

## Notable Entry Points

- `wallet_transport::client::HttpClient`
- `wallet_transport::client::RpcClient`
- `wallet_transport::request_builder::ReqBuilder`
- `wallet_transport::TransportError`
- `wallet_transport::types`

## Notes

- The higher-level chain interaction code relies on this crate for outbound
  network requests.
- Network behavior is intentionally isolated here so chain logic can stay more
  focused on protocol details.
