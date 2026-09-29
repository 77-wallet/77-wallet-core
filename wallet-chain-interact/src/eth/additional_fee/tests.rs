use crate::{
    eth::{EthChain, Provider, operations::TransferOpt},
    types::Transaction as _,
};
use alloy::{
    network::TransactionBuilder,
    primitives::{Address, U256, keccak256},
    rpc::types::TransactionRequest,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
use wallet_transport::client::RpcClient;
use wallet_types::chain::{chain::ChainCode, network::NetworkKind};

fn chain(url: &str, headers: Option<HashMap<String, String>>, code: ChainCode) -> EthChain {
    let client = RpcClient::new(url, headers, Some(Duration::from_secs(5))).unwrap();
    EthChain::new(Provider::new(client).unwrap(), NetworkKind::Mainnet, code).unwrap()
}

fn rpc(result: Value) -> (String, thread::JoinHandle<(Value, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0_u8; 4096];
        let (body_start, content_length, headers) = loop {
            let count = socket.read(&mut buf).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buf[..count]);
            if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                let headers = std::str::from_utf8(&bytes[..end]).unwrap().to_string();
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (end + 4, content_length, headers);
            }
        };
        while bytes.len() < body_start + content_length {
            let count = socket.read(&mut buf).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&buf[..count]);
        }
        let request: Value =
            serde_json::from_slice(&bytes[body_start..body_start + content_length]).unwrap();
        let response = if result.get("error").is_some() {
            json!({"jsonrpc":"2.0","id":request["id"],"error":result["error"]})
        } else {
            json!({"jsonrpc":"2.0","id":request["id"],"result":result})
        }
        .to_string();
        write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        (request, headers)
    });
    (url, worker)
}

fn native_tx() -> TransactionRequest {
    let to: Address = "0x0000000000000000000000000000000000000002".parse().unwrap();
    TransactionRequest::default().to(to).with_value(U256::from(1_u64))
}

fn erc20_tx() -> TransactionRequest {
    TransferOpt::new(
        "0x0000000000000000000000000000000000000001",
        "0x0000000000000000000000000000000000000002",
        U256::from(10_u64),
        Some("0x0000000000000000000000000000000000000003".to_string()),
    )
    .unwrap()
    .build_transaction()
    .unwrap()
}

#[tokio::test]
async fn evm_additional_fee_non_l2_chains_do_not_call_rpc() {
    for code in [
        ChainCode::Ethereum,
        ChainCode::BnbSmartChain,
        ChainCode::Polygon,
        ChainCode::Avalanche,
        ChainCode::Arbitrum,
    ] {
        let fee = chain("http://127.0.0.1:1", None, code)
            .estimate_additional_fee(native_tx())
            .await
            .unwrap();
        assert_eq!(fee, None, "unexpected additional fee for {code}");
    }
}

#[tokio::test]
async fn evm_additional_fee_scroll_native_uses_same_provider_and_auth() {
    let (url, worker) = rpc(json!("0x38d7ea4c68000"));
    let headers = HashMap::from([("x-wallet-rpc-auth".to_string(), "test-only".to_string())]);
    let fee = chain(&url, Some(headers), ChainCode::Scroll)
        .estimate_additional_fee(native_tx())
        .await
        .unwrap();
    let (request, received_headers) = worker.join().unwrap();
    assert_eq!(fee, Some(U256::from(1_000_000_000_000_000_u64)));
    assert_eq!(request["method"], "scroll_estimateL1DataFee");
    assert_eq!(request["params"][0]["to"], "0x0000000000000000000000000000000000000002");
    assert_eq!(request["params"][0]["value"], "0x1");
    assert_eq!(request["params"][1], "latest");
    assert!(received_headers.to_ascii_lowercase().contains("x-wallet-rpc-auth: test-only"));
}

#[tokio::test]
async fn evm_additional_fee_scroll_erc20_contains_transfer_calldata() {
    let (url, worker) = rpc(json!("0x1"));
    let fee =
        chain(&url, None, ChainCode::Scroll).estimate_additional_fee(erc20_tx()).await.unwrap();
    let (request, _) = worker.join().unwrap();
    assert_eq!(fee, Some(U256::from(1_u64)));
    assert_eq!(request["params"][0]["to"], "0x0000000000000000000000000000000000000003");
    let data = request["params"][0]["input"].as_str().unwrap();
    assert!(data.starts_with("0xa9059cbb"));
    assert!(data.ends_with("000000000000000000000000000000000000000000000000000000000000000a"));
}

#[tokio::test]
async fn evm_additional_fee_base_native_and_erc20_query_oracle_upper_bound() {
    for (tx, expected_size) in [(native_tx(), 160), (erc20_tx(), 228)] {
        let (url, worker) = rpc(json!(format!("0x{:064x}", 42_u64)));
        let fee = chain(&url, None, ChainCode::Base).estimate_additional_fee(tx).await.unwrap();
        let (request, _) = worker.join().unwrap();
        assert_eq!(fee, Some(U256::from(42_u64)));
        assert_eq!(request["method"], "eth_call");
        assert_eq!(request["params"][0]["to"], "0x420000000000000000000000000000000000000f");
        assert_eq!(request["params"][1], "latest");
        let data = request["params"][0]["data"].as_str().unwrap();
        let selector = keccak256(b"getL1FeeUpperBound(uint256)");
        assert_eq!(&data[..10], format!("0x{}", hex::encode(&selector[..4])));
        assert_eq!(u64::from_str_radix(&data[data.len() - 64..], 16).unwrap(), expected_size);
    }
}

#[tokio::test]
async fn evm_additional_fee_invalid_zero_and_rpc_errors_fail() {
    for (code, result) in [
        (ChainCode::Scroll, json!("not-a-hex-fee")),
        (ChainCode::Scroll, json!("0x0")),
        (ChainCode::Scroll, json!({"error":{"code":-32601,"message":"method not found"}})),
        (ChainCode::Base, json!("invalid")),
        (ChainCode::Base, json!(format!("0x{:064x}", 0_u64))),
        (ChainCode::Base, json!({"error":{"code":-32000,"message":"oracle unavailable"}})),
    ] {
        let (url, worker) = rpc(result);
        let quote = chain(&url, None, code).estimate_additional_fee(native_tx()).await;
        worker.join().unwrap();
        assert!(quote.is_err(), "{code} must not hide an additional-fee failure");
    }
}
