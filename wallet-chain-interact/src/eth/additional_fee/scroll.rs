use crate::{Error, eth::Provider};
use alloy::{primitives::U256, rpc::types::TransactionRequest};
use serde_json::json;
use wallet_transport::types::JsonRpcParams;
use wallet_utils::unit;

pub(super) async fn estimate_l1_data_fee(
    provider: &Provider,
    tx: TransactionRequest,
) -> crate::Result<U256> {
    let params = JsonRpcParams::default()
        .method("scroll_estimateL1DataFee")
        .params(vec![json!(tx), json!("latest")]);
    let result = provider.additional_fee_rpc(params).await?;
    let fee = unit::u256_from_str(&result)?;
    if fee == U256::ZERO {
        return Err(Error::RpcError("Scroll L1 data fee RPC returned zero".to_string()));
    }
    Ok(fee)
}
