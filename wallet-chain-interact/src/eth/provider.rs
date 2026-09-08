use super::{
    EtherFee,
    protocol::contract::{isBlackListedCall, nameCall, symbolCall},
};
use crate::eth::protocol::contract::{balanceOfCall, decimalsCall};
use alloy::{
    network::{TransactionBuilder, eip2718::Encodable2718},
    primitives::U256,
    rpc::types::{Block, TransactionInput, TransactionReceipt, TransactionRequest},
    sol_types::SolCall,
};
use serde_json::json;
use wallet_transport::{client::RpcClient, types::JsonRpcParams};
use wallet_types::chain::chain::ChainCode;
use wallet_utils::{address, unit};

pub struct Provider {
    client: RpcClient,
}

impl Provider {
    pub fn new(rpc_client: RpcClient) -> crate::Result<Self> {
        Ok(Self { client: rpc_client })
    }

    pub async fn balance(&self, addr: &str) -> crate::Result<U256> {
        let params = JsonRpcParams::default()
            .method("eth_getBalance")
            .params(vec![addr, "latest"]);

        let r = self.client.invoke_request::<_, String>(params).await?;
        Ok(unit::u256_from_str(&r)?)
    }

    pub async fn is_contract_address(&self, addr: &str) -> crate::Result<String> {
        let params = JsonRpcParams::default()
            .method("eth_getCode")
            .params(vec![addr, "latest"]);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn estimate_gas(&self, tx: TransactionRequest) -> crate::Result<U256> {
        let params = JsonRpcParams::default()
            .method("eth_estimateGas")
            .params(vec![json!(tx)]);

        let r = self.client.invoke_request::<_, String>(params).await?;

        let gas = wallet_utils::unit::u256_from_str(&r)?;
        let ten_percent = (gas * U256::from(10)) / U256::from(100);

        Ok(gas + ten_percent)
    }

    // used testnet
    pub async fn get_fee(
        &self,
        tx: TransactionRequest,
    ) -> crate::Result<super::params::FeeSetting> {
        let gas_limit = self.estimate_gas(tx.clone()).await?;

        let max_priority_fee_per_gas = U256::from(2_000_000_000u64);
        let gas_price = self.gas_price().await?;
        let max_fee = gas_price + max_priority_fee_per_gas;
        let res = super::params::FeeSetting {
            base_fee: gas_price,
            gas_limit,
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas,
        };
        Ok(res)
    }

    pub async fn set_transaction_fee(
        &self,
        tx: TransactionRequest,
        fee: super::params::FeeSetting,
        chain_code: ChainCode,
    ) -> crate::Result<TransactionRequest> {
        // 币安链 单独处理
        let max_fee = match chain_code {
            ChainCode::BnbSmartChain => fee.max_priority_fee_per_gas.to::<u128>(),
            _ => fee.max_fee_per_gas.to::<u128>(),
        };

        Ok(tx
            .with_gas_limit(fee.gas_limit.to::<u64>())
            .with_max_priority_fee_per_gas(fee.max_priority_fee_per_gas.to::<u128>())
            .with_max_fee_per_gas(max_fee))
    }

    pub async fn build_signed_raw_transaction(
        &self,
        tx: TransactionRequest,
        key: &str,
        nonce: Option<u64>,
    ) -> crate::Result<(Vec<u8>, String)> {
        // 1. nonce
        let n = match nonce {
            Some(n) => n,
            None => self.nonce(&tx.from.clone().unwrap().to_string()).await?,
        };

        // 2. chain id
        let chain_id = self.chain_id().await?;
        let tx = tx.with_nonce(n).with_chain_id(chain_id);

        Self::sign_transaction(tx, key).await
    }

    async fn sign_transaction(
        tx: TransactionRequest,
        key: &str,
    ) -> crate::Result<(Vec<u8>, String)> {
        let signer: alloy::signers::local::PrivateKeySigner = key
            .parse()
            .map_err(|_| crate::Error::SignError("parse key failed".to_string()))?;
        let wallet = alloy::network::EthereumWallet::from(signer);

        let tx_envelope = tx
            .build(&wallet)
            .await
            .map_err(|e| crate::Error::SignError(e.to_string()))?;

        let bytes = tx_envelope.encoded_2718();

        // 无需 decode/encode，直接用包含签名的 envelope 计算 hash
        let tx_hash = format!("0x{}", hex::encode(tx_envelope.tx_hash().as_slice()));

        Ok((bytes.to_vec(), tx_hash))
    }

    pub async fn broadcast_raw_transaction(&self, raw: &[u8]) -> crate::Result<String> {
        let hex_raw = format!("0x{}", hex::encode(raw));
        let params = JsonRpcParams::default()
            .method("eth_sendRawTransaction")
            .params(vec![hex_raw]);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn send_raw_transaction(
        &self,
        tx: TransactionRequest,
        key: &str,
        nonce: Option<u64>,
    ) -> crate::Result<String> {
        let n = if nonce.is_none() {
            self.nonce(&tx.from.unwrap().to_string()).await?
        } else {
            nonce.unwrap()
        };

        let chain_id = self.chain_id().await?;
        let tx = tx.with_nonce(n).with_chain_id(chain_id);

        // 签名交易
        let signer: alloy::signers::local::PrivateKeySigner = key
            .parse()
            .map_err(|_| crate::Error::SignError("get singer from key error".to_string()))?;
        let wallet = alloy::network::EthereumWallet::from(signer);

        let tx_envelope = tx
            .build(&wallet)
            .await
            .map_err(|e| crate::Error::SignError(e.to_string()))?;
        tx_envelope.encoded_2718();
        let tx_encoded = tx_envelope.encoded_2718();

        let hex_raw = format!("0x{}", hex::encode(tx_encoded));
        let params = JsonRpcParams::default()
            .method("eth_sendRawTransaction")
            .params(vec![hex_raw]);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn token_balance(&self, addr: &str, token: &str) -> crate::Result<U256> {
        let token_addr = address::parse_eth_address(token)?;
        let addr = address::parse_eth_address(addr)?;

        let call = balanceOfCall { owner: addr };
        let data = call.abi_encode();

        let input = TransactionInput {
            input: None,
            data: Some(data.into()),
        };
        let tx_req = TransactionRequest::default().to(token_addr).input(input);

        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx_req), "latest".into()]);

        let r = self.client.invoke_request::<_, String>(params).await?;
        Ok(wallet_utils::unit::u256_from_str(&r)?)
    }

    pub async fn eth_call(&self, tx: TransactionRequest) -> crate::Result<String> {
        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx), json!("latest")]);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn get_block_height(&self) -> crate::Result<String> {
        let params: JsonRpcParams<()> = JsonRpcParams::default()
            .method("eth_blockNumber")
            .no_params();

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    // 代币精度
    pub async fn decimals(&self, token: &str) -> crate::Result<U256> {
        let token_addr = address::parse_eth_address(token)?;

        let call = decimalsCall {};
        let data = call.abi_encode();

        let input = TransactionInput {
            input: None,
            data: Some(data.into()),
        };
        let tx_req = TransactionRequest::default().to(token_addr).input(input);

        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx_req), json!("latest")]);

        let r = self.client.invoke_request::<_, String>(params).await?;
        Ok(unit::u256_from_str(&r)?)
    }

    pub async fn token_symbol(&self, token: &str) -> crate::Result<String> {
        let token_addr = address::parse_eth_address(token)?;

        let call = symbolCall {};
        let data = call.abi_encode();

        let input = TransactionInput {
            input: None,
            data: Some(data.into()),
        };
        let tx_req = TransactionRequest::default().to(token_addr).input(input);

        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx_req), json!("latest")]);

        let r = self.client.invoke_request::<_, String>(params).await?;

        Ok(r)
    }

    pub async fn token_name(&self, token: &str) -> crate::Result<String> {
        let token_addr = address::parse_eth_address(token)?;

        let call = nameCall {};
        let data = call.abi_encode();

        let input = TransactionInput {
            input: None,
            data: Some(data.into()),
        };
        let tx_req = TransactionRequest::default().to(token_addr).input(input);

        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx_req), json!("latest")]);

        let r = self.client.invoke_request::<_, String>(params).await?;
        Ok(r)
    }

    pub async fn black_address(&self, token: &str, owner: &str) -> crate::Result<String> {
        let token_addr = address::parse_eth_address(token)?;
        let owner_addr = address::parse_eth_address(owner)?;

        let call = isBlackListedCall { from: owner_addr };
        let data = call.abi_encode();

        let input = TransactionInput {
            input: None,
            data: Some(data.into()),
        };
        let tx_req = TransactionRequest::default().to(token_addr).input(input);

        let params = JsonRpcParams::default()
            .method("eth_call")
            .params(vec![json!(tx_req), json!("latest")]);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn gas_price(&self) -> crate::Result<U256> {
        let params = JsonRpcParams::<Vec<String>>::default()
            .method("eth_gasPrice")
            .params(vec![]);

        let r = self.client.invoke_request::<_, String>(params).await?;
        Ok(unit::u256_from_str(&r)?)
    }

    // 最后区块
    pub async fn latest_block(&self) -> crate::Result<Block> {
        let params = JsonRpcParams::default()
            .method("eth_getBlockByNumber")
            .params(vec![json!("latest"), json!(false)]);

        Ok(self.client.invoke_request::<_, Block>(params).await?)
    }

    // price unit is wei
    pub async fn get_default_fee(&self) -> crate::Result<EtherFee> {
        let block = self.latest_block().await?;

        let base_fee = block
            .header
            .base_fee_per_gas
            .map(|base_fee| U256::from(base_fee).max(U256::ZERO))
            .unwrap_or(U256::ZERO);

        let gas_price = self.gas_price().await?;
        let priority_fee_per_gas = gas_price.saturating_sub(base_fee);

        Ok(EtherFee {
            base_fee,
            priority_fee_per_gas,
        })
    }

    pub async fn base_fee(&self) -> crate::Result<U256> {
        let block = self.latest_block().await?;

        match block.header.base_fee_per_gas {
            Some(base_fee) => {
                if base_fee > 0 {
                    Ok(U256::from(base_fee))
                } else {
                    self.gas_price().await
                }
            }
            None => self.gas_price().await,
        }
    }

    pub async fn nonce(&self, addr: &str) -> crate::Result<u64> {
        let params = JsonRpcParams::default()
            .method("eth_getTransactionCount")
            .params(vec![addr, "pending"]);

        let rs = self.client.invoke_request::<_, String>(params).await?;

        let nonce = wallet_utils::unit::u256_from_str(&rs)?;
        Ok(nonce.to::<u64>())
    }

    pub async fn chain_id(&self) -> crate::Result<u64> {
        let c: Vec<String> = Vec::with_capacity(1);
        let params = JsonRpcParams::default().method("eth_chainId").params(c);

        let rs = self.client.invoke_request::<_, String>(params).await?;
        let chain_id = wallet_utils::unit::u256_from_str(&rs)?;

        Ok(chain_id.to::<u64>())
    }

    // 查询交易的收据
    pub async fn transaction_receipt(&self, hash: &str) -> crate::Result<TransactionReceipt> {
        let params = JsonRpcParams::default()
            .method("eth_getTransactionReceipt")
            .params(vec![hash]);

        Ok(self
            .client
            .invoke_request::<_, TransactionReceipt>(params)
            .await?)
    }

    pub async fn block_by_hash(&self, hash: &str) -> crate::Result<Block> {
        let params = JsonRpcParams::default()
            .method("eth_getBlockByHash")
            .params(vec![json!(hash), json!(false)]);

        let result = self.client.invoke_request::<_, Block>(params).await?;
        Ok(result)
    }

    pub async fn block_by_num(&self, num: i64) -> crate::Result<Block> {
        let params = JsonRpcParams::default()
            .method("eth_getBlockByNumber")
            .params(vec![json!(num), json!(true)]);

        let result = self.client.invoke_request::<_, Block>(params).await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::Provider;
    use crate::{eth::operations::TransferOpt, types::Transaction as _};
    use alloy::{network::TransactionBuilder as _, primitives::U256};

    const TEST_PRIVATE_KEY: &str =
        "0000000000000000000000000000000000000000000000000000000000000001";
    const FROM: &str = "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf";
    const TO: &str = "0x2222222222222222222222222222222222222222";
    const TOKEN: &str = "0x3333333333333333333333333333333333333333";
    const TEST_CHAIN_ID: u64 = 84_530;
    const NONCE: u64 = 7;

    async fn sign_transfer(contract: Option<String>) -> (String, String) {
        let gas_limit = if contract.is_some() { 65_000 } else { 21_000 };
        let tx = TransferOpt::new(FROM, TO, U256::from(987_654_321u64), contract)
            .unwrap()
            .build_transaction()
            .unwrap()
            .with_gas_limit(gas_limit)
            .with_max_priority_fee_per_gas(1_500_000_000u128)
            .with_max_fee_per_gas(30_000_000_000u128)
            .with_nonce(NONCE)
            .with_chain_id(TEST_CHAIN_ID);

        let (raw, hash) = Provider::sign_transaction(tx, TEST_PRIVATE_KEY)
            .await
            .unwrap();
        (format!("0x{}", hex::encode(raw)), hash)
    }

    #[tokio::test]
    async fn evm_eip1559_native_signing_vector_is_stable() {
        let (raw, hash) = sign_transfer(None).await;
        assert!(raw.starts_with("0x02"));
        assert_eq!(
            raw,
            "0x02f87283014a32078459682f008506fc23ac00825208942222222222222222222222222222222222222222843ade68b180c001a0e79b603f5408f017b2fbd8bb2ee7e98490bde1a69ca35bcb7bc2497d11657607a00a52d4d7808cd705a2b4cbc2c363efd14966a83ba603fd5a2a2ca3d5ec9cf128"
        );
        assert_eq!(
            hash,
            "0xf482a27dbfadda329b0d42b56617ee37e747079ae8fc4677e16cd12439b765e1"
        );
    }

    #[tokio::test]
    async fn evm_eip1559_erc20_signing_vector_is_stable() {
        let (raw, hash) = sign_transfer(Some(TOKEN.to_string())).await;
        assert!(raw.starts_with("0x02"));
        assert_eq!(
            raw,
            "0x02f8b383014a32078459682f008506fc23ac0082fde894333333333333333333333333333333333333333380b844a9059cbb0000000000000000000000002222222222222222222222222222222222222222000000000000000000000000000000000000000000000000000000003ade68b1c080a007f5672fff804c3f586b0e853f0c149b88fa2b3dd37a8aa93155db7fc075b661a06f6a637b9fdd117536a6fc72e20800614eaea760e46b58435f276fb3109880d3"
        );
        assert_eq!(
            hash,
            "0x8228153a938440610a562877204e871147d56e576eb2bed9290005d6cd23d6bf"
        );
    }
}
