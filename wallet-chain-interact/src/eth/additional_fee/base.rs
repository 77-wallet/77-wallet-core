use crate::{Error, eth::Provider};
use alloy::{
    primitives::{TxKind, U256, keccak256},
    rpc::types::TransactionRequest,
};
use serde_json::json;
use wallet_transport::types::JsonRpcParams;

const GAS_PRICE_ORACLE: &str = "0x420000000000000000000000000000000000000f";
// Bounds the unsigned EIP-1559 fields of a plain transfer (chain ID, nonce,
// fees, gas, recipient, value, and RLP framing). The oracle adds signatures;
// calldata is counted separately. Extended transaction shapes are rejected.
const MAX_UNSIGNED_EIP1559_ENVELOPE_BYTES: usize = 256;

pub(super) async fn estimate_l1_data_fee(
    provider: &Provider,
    tx: TransactionRequest,
) -> crate::Result<U256> {
    let input = tx.input.input().map_or(&[][..], |input| input.as_ref());
    let is_native = input.is_empty();
    let is_erc20_transfer = input.len() == 68
        && input.starts_with(&keccak256(b"transfer(address,uint256)")[..4])
        && tx.value.unwrap_or_default().is_zero();
    if tx.from.is_none()
        || !matches!(tx.to, Some(TxKind::Call(_)))
        || !(is_native || is_erc20_transfer)
        || tx.access_list.is_some()
        || tx.gas_price.is_some()
        || tx.transaction_type.is_some_and(|tx_type| tx_type != 2)
        || tx.max_fee_per_blob_gas.is_some()
        || tx.blob_versioned_hashes.is_some()
        || tx.sidecar.is_some()
        || tx.authorization_list.is_some()
    {
        return Err(Error::Other("unsupported Base transaction for L1 fee estimate".to_string()));
    }
    let input_len = input.len();
    let unsigned_size = input_len
        .checked_add(MAX_UNSIGNED_EIP1559_ENVELOPE_BYTES)
        .ok_or_else(|| Error::Other("Base unsigned transaction size overflow".to_string()))?;

    let selector = keccak256(b"getL1FeeUpperBound(uint256)");
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(&selector[..4]);
    calldata.extend_from_slice(&U256::from(unsigned_size).to_be_bytes::<32>());
    let params = JsonRpcParams::default().method("eth_call").params(vec![
        json!({"to": GAS_PRICE_ORACLE, "data": format!("0x{}", hex::encode(calldata))}),
        json!("latest"),
    ]);
    let result = provider.additional_fee_rpc(params).await?;
    let encoded = result.strip_prefix("0x").ok_or_else(|| {
        Error::RpcError("Base L1 fee oracle returned a non-hex value".to_string())
    })?;
    if encoded.len() != 64 {
        return Err(Error::RpcError("Base L1 fee oracle returned an invalid uint256".to_string()));
    }
    let bytes = hex::decode(encoded)
        .map_err(|_| Error::RpcError("Base L1 fee oracle returned invalid hex".to_string()))?;
    let fee = U256::from_be_slice(&bytes);
    if fee == U256::ZERO {
        return Err(Error::RpcError("Base L1 fee oracle returned zero".to_string()));
    }
    Ok(fee)
}
