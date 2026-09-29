use super::{
    EthChain,
    operations::multisig::{MULTISIG_FACTORY, SAFE},
};
use wallet_types::chain::{chain::ChainCode, network::NetworkKind};

impl EthChain {
    /// Check the selected RPC before deriving or deploying a supported Safe.
    pub async fn preflight_safe_deployment(&self) -> crate::Result<()> {
        let expected_chain_id = match (self.chain_code, self.network) {
            (ChainCode::Polygon, NetworkKind::Mainnet) => 137,
            (ChainCode::Avalanche, NetworkKind::Mainnet) => 43114,
            _ => {
                return Err(crate::Error::Other(format!(
                    "Safe deployment preflight is unsupported on {} {:?}",
                    self.chain_code, self.network
                )));
            }
        };

        let actual_chain_id = self.provider.chain_id().await?;
        if actual_chain_id != expected_chain_id {
            return Err(crate::Error::Other(format!(
                "{} Safe RPC returned chain ID {actual_chain_id}, expected {expected_chain_id}",
                self.chain_code
            )));
        }

        for (name, address) in [("Safe factory", MULTISIG_FACTORY), ("Safe singleton", SAFE)] {
            let code = self.provider.is_contract_address(address).await?;
            if !code.starts_with("0x") || code.len() <= 2 || hex::decode(&code[2..]).is_err() {
                return Err(crate::Error::Other(format!(
                    "{} {name} has no valid contract code at {address}",
                    self.chain_code
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eth::Provider;
    use serde_json::{Value, json};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    use wallet_transport::client::RpcClient;

    fn fake_rpc(
        chain_code: ChainCode,
        responses: Vec<(&'static str, &'static str)>,
    ) -> (EthChain, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let mut calls = Vec::new();
            for (expected_method, result) in responses {
                let (mut socket, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let (body_start, body_len) = loop {
                    let n = socket.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header = std::str::from_utf8(&bytes[..end]).unwrap();
                        let len = header
                            .lines()
                            .find_map(|line| {
                                let (key, value) = line.split_once(':')?;
                                key.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        break (end + 4, len);
                    }
                };
                while bytes.len() < body_start + body_len {
                    let n = socket.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let request: Value =
                    serde_json::from_slice(&bytes[body_start..body_start + body_len]).unwrap();
                let method = request["method"].as_str().unwrap();
                assert_eq!(method, expected_method);
                if method == "eth_getCode" {
                    let expected_address = match calls.len() {
                        1 => MULTISIG_FACTORY,
                        2 => SAFE,
                        _ => panic!("unexpected Safe contract lookup"),
                    };
                    assert_eq!(
                        request["params"][0].as_str().unwrap().to_lowercase(),
                        expected_address.to_lowercase()
                    );
                }
                calls.push(method.to_owned());
                let response =
                    json!({"jsonrpc":"2.0", "id":request["id"], "result":result}).to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
            calls
        });
        let client = RpcClient::new(&url, None, None).unwrap();
        let chain = EthChain::new(
            Provider::new(client).unwrap(),
            NetworkKind::Mainnet,
            chain_code,
        )
        .unwrap();
        (chain, worker)
    }

    fn fake_polygon_rpc(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (EthChain, thread::JoinHandle<Vec<String>>) {
        fake_rpc(ChainCode::Polygon, responses)
    }

    #[tokio::test]
    async fn polygon_safe_preflight_accepts_matching_rpc_and_contracts() {
        let (chain, worker) = fake_polygon_rpc(vec![
            ("eth_chainId", "0x89"),
            ("eth_getCode", "0x6001"),
            ("eth_getCode", "0x6002"),
        ]);
        chain.preflight_safe_deployment().await.unwrap();
        assert_eq!(
            worker.join().unwrap(),
            ["eth_chainId", "eth_getCode", "eth_getCode"]
        );
    }

    #[tokio::test]
    async fn polygon_safe_preflight_rejects_wrong_chain_before_contract_calls() {
        let (chain, worker) = fake_polygon_rpc(vec![("eth_chainId", "0x1")]);
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("expected 137")
        );
        assert_eq!(worker.join().unwrap(), ["eth_chainId"]);
    }

    #[tokio::test]
    async fn polygon_safe_preflight_rejects_missing_factory() {
        let (chain, worker) =
            fake_polygon_rpc(vec![("eth_chainId", "0x89"), ("eth_getCode", "0x")]);
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("factory")
        );
        assert_eq!(worker.join().unwrap(), ["eth_chainId", "eth_getCode"]);
    }

    #[tokio::test]
    async fn polygon_safe_preflight_rejects_missing_singleton() {
        let (chain, worker) = fake_polygon_rpc(vec![
            ("eth_chainId", "0x89"),
            ("eth_getCode", "0x6001"),
            ("eth_getCode", "0x"),
        ]);
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("singleton")
        );
        assert_eq!(
            worker.join().unwrap(),
            ["eth_chainId", "eth_getCode", "eth_getCode"]
        );
    }

    #[tokio::test]
    async fn polygon_safe_preflight_rejects_malformed_code() {
        let (chain, worker) =
            fake_polygon_rpc(vec![("eth_chainId", "0x89"), ("eth_getCode", "0x1")]);
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("factory")
        );
        assert_eq!(worker.join().unwrap(), ["eth_chainId", "eth_getCode"]);
    }

    #[tokio::test]
    async fn avalanche_safe_preflight_accepts_matching_rpc_and_contracts() {
        let (chain, worker) = fake_rpc(
            ChainCode::Avalanche,
            vec![
                ("eth_chainId", "0xa86a"),
                ("eth_getCode", "0x6001"),
                ("eth_getCode", "0x6002"),
            ],
        );
        chain.preflight_safe_deployment().await.unwrap();
        assert_eq!(
            worker.join().unwrap(),
            ["eth_chainId", "eth_getCode", "eth_getCode"]
        );
    }

    #[tokio::test]
    async fn avalanche_safe_preflight_rejects_wrong_chain_before_contract_calls() {
        let (chain, worker) = fake_rpc(ChainCode::Avalanche, vec![("eth_chainId", "0x89")]);
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("expected 43114")
        );
        assert_eq!(worker.join().unwrap(), ["eth_chainId"]);
    }

    #[tokio::test]
    async fn avalanche_safe_preflight_rejects_missing_singleton() {
        let (chain, worker) = fake_rpc(
            ChainCode::Avalanche,
            vec![
                ("eth_chainId", "0xa86a"),
                ("eth_getCode", "0x6001"),
                ("eth_getCode", "0x"),
            ],
        );
        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("singleton")
        );
        assert_eq!(
            worker.join().unwrap(),
            ["eth_chainId", "eth_getCode", "eth_getCode"]
        );
    }

    #[tokio::test]
    async fn avalanche_testnet_remains_unsupported_without_rpc_call() {
        let client = RpcClient::new("http://127.0.0.1:1", None, None).unwrap();
        let provider = Provider::new(client).unwrap();
        let chain = EthChain::new(provider, NetworkKind::Testnet, ChainCode::Avalanche).unwrap();

        assert!(
            chain
                .preflight_safe_deployment()
                .await
                .unwrap_err()
                .to_string()
                .contains("unsupported")
        );
    }
}
