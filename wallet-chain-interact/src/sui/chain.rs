use super::consts::{self, DEFAULT_GAS_BUDGET, SUI_VALUE};
use super::error::{ExecutionPhase, SuiError};
use super::protocol::{EstimateFeeResp, SuiQueriedTransaction, SuiSimulation};
use super::provider::Provider;
use crate::types::ChainPrivateKey;
use crate::{BillResourceConsume, QueryTransactionResult};
use alloy::primitives::U256;
use sui_crypto::{SuiSigner, ed25519::Ed25519PrivateKey};
use sui_sdk_types::bcs::FromBcs;
use sui_sdk_types::{ProgrammableTransaction, Transaction};

pub struct SuiChain {
    pub provider: Provider,
}

impl SuiChain {
    pub fn new(provider: Provider) -> crate::Result<Self> {
        Ok(Self { provider })
    }
}

impl SuiChain {
    pub async fn balance(&self, addr: &str, token: Option<String>) -> crate::Result<U256> {
        let coin_type = if let Some(t) = &token {
            t
        } else {
            consts::SUI_NATIVE_COIN
        };

        let res = self.provider.balance(addr, &coin_type).await?;

        Ok(U256::from(res.total_balance))
    }

    pub async fn block_num(&self) -> crate::Result<u64> {
        let latest_block = self.provider.latest_block().await?;

        Ok(wallet_utils::parse_func::u64_from_str(&latest_block)?)
    }

    pub async fn query_tx_res(
        &self,
        digest: &str,
    ) -> crate::Result<Option<QueryTransactionResult>> {
        let tx = self.provider.query_tx_info(digest).await?;

        let transaction_time = u128::from(tx.timestamp_seconds);
        let transaction_fee = Self::extract_gas_used(&tx).ok_or_else(|| {
            SuiError::invalid_response("query transaction", "gas_price", "must fit i64")
        })?;
        let status = Self::extract_status(&tx);
        let block_height = u128::from(tx.checkpoint);

        let resource_consume =
            BillResourceConsume::one_resource(transaction_fee.1 as u64).to_json_str()?;

        let result = QueryTransactionResult::new(
            tx.digest.clone(),
            transaction_fee.0,
            resource_consume,
            transaction_time,
            status,
            block_height,
        );

        Ok(Some(result))
    }

    pub async fn decimals(&self, token_addr: &str) -> crate::Result<u8> {
        let meta_data = self.provider.get_coin_metadata(token_addr).await?;
        Ok(meta_data.decimals)
    }

    pub async fn token_symbol(&self, token: &str) -> crate::Result<String> {
        let meta_data = self.provider.get_coin_metadata(token).await?;
        Ok(meta_data.symbol)
    }

    pub async fn token_name(&self, token: &str) -> crate::Result<String> {
        let meta_data = self.provider.get_coin_metadata(token).await?;
        Ok(meta_data.name)
    }

    pub async fn black_address(&self, coin_type: &str, owner: &str) -> crate::Result<bool> {
        self.provider.black_address(coin_type, owner).await
    }

    pub async fn estimate_fee(
        &self,
        sender: &str,
        tx: ProgrammableTransaction,
    ) -> crate::Result<EstimateFeeResp> {
        let gas_price = self.provider.get_reference_gas_price().await?;

        let result = self
            .provider
            .dev_inspect_transaction(sender, tx, gas_price)
            .await?;

        Self::estimate_fee_from_simulation(result, gas_price)
    }

    fn estimate_fee_from_simulation(
        result: SuiSimulation,
        gas_price: u64,
    ) -> crate::Result<EstimateFeeResp> {
        if !result.success {
            return Err(SuiError::ExecutionFailed {
                phase: ExecutionPhase::Simulation,
                reason: result
                    .error
                    .unwrap_or_else(|| "transaction execution failed".into()),
            }
            .into());
        }

        let gas_used = result
            .computation_cost
            .checked_add(result.storage_cost)
            .ok_or_else(|| SuiError::TransactionBuild {
                stage: "estimate gas",
                reason: "gas cost overflow".into(),
            })?;
        let gas_used = if gas_used == 0 {
            DEFAULT_GAS_BUDGET
        } else {
            gas_used
        };

        Ok(EstimateFeeResp::new(gas_used, gas_price))
    }

    pub async fn exec(
        &self,
        tx_data: Transaction,
        private_key: ChainPrivateKey,
    ) -> crate::Result<String> {
        let tx_bytes = wallet_utils::serde_func::bcs_to_bytes(&tx_data)?;
        let signature = Self::sign_transaction(&tx_bytes, private_key)?;

        self.provider
            .send_transaction(tx_bytes, vec![signature])
            .await
    }

    fn sign_transaction(tx_bytes: &[u8], private_key: ChainPrivateKey) -> crate::Result<Vec<u8>> {
        let transaction = sui_sdk_types::Transaction::from_bcs(tx_bytes).map_err(|error| {
            crate::Error::SignError(format!("invalid Sui transaction BCS: {error}"))
        })?;
        let key_bytes: [u8; Ed25519PrivateKey::LENGTH] = private_key
            .to_bytes()?
            .try_into()
            .map_err(|_| crate::Error::SignError("Sui private key must be 32 bytes".into()))?;
        let key = Ed25519PrivateKey::new(key_bytes);
        let signature = key
            .sign_transaction(&transaction)
            .map_err(|error| crate::Error::SignError(format!("Sui signing failed: {error}")))?;
        Ok(signature.to_bytes())
    }
}

impl SuiChain {
    pub fn extract_gas_used(resp: &SuiQueriedTransaction) -> Option<(f64, i64)> {
        let gas_price = i64::try_from(resp.gas_price).ok()?;
        // Gasless transactions have no price-derived gas units. Fees still come
        // from effects rather than multiplying by the declared gas price.
        let gas_used = if gas_price == 0 {
            0
        } else {
            resp.net_gas_usage / gas_price
        };
        Some((resp.net_gas_usage as f64 / SUI_VALUE, gas_used))
    }

    pub fn extract_status(resp: &SuiQueriedTransaction) -> i8 {
        if resp.success { 2 } else { 3 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use sui_crypto::SuiVerifier;
    use sui_sdk_types::{GasPayment, TransactionExpiration, TransactionKind};

    const TEST_ADDRESS: &str = "0x885f29a4f1b4d63822728a1b1811d0278c4e25f27d3754ddd387cd34f9482d0f";

    fn transaction_bytes() -> Vec<u8> {
        let sender = TEST_ADDRESS.parse().unwrap();
        let transaction = Transaction {
            kind: TransactionKind::ProgrammableTransaction(ProgrammableTransaction {
                inputs: Vec::new(),
                commands: Vec::new(),
            }),
            sender,
            gas_payment: GasPayment {
                objects: Vec::new(),
                owner: sender,
                price: 1,
                budget: 1000,
            },
            expiration: TransactionExpiration::None,
        };
        wallet_utils::serde_func::bcs_to_bytes(&transaction).unwrap()
    }

    fn generated_private_key() -> ([u8; Ed25519PrivateKey::LENGTH], ChainPrivateKey) {
        let mut key_bytes = [0; Ed25519PrivateKey::LENGTH];
        rand::rngs::OsRng.fill_bytes(&mut key_bytes);
        let private_key = ChainPrivateKey::from(hex::encode(key_bytes));
        (key_bytes, private_key)
    }

    #[test]
    fn estimate_fee_sums_computation_and_storage_costs() {
        let simulation = SuiSimulation {
            success: true,
            error: None,
            computation_cost: 100,
            storage_cost: 20,
        };
        let estimate = SuiChain::estimate_fee_from_simulation(simulation, 1000).unwrap();

        assert_eq!(estimate.gas_fee, 120);
        assert_eq!(estimate.gas_price, 1000);
    }

    #[test]
    fn estimate_fee_uses_default_budget_for_zero_cost() {
        let simulation = SuiSimulation {
            success: true,
            error: None,
            computation_cost: 0,
            storage_cost: 0,
        };

        assert_eq!(
            SuiChain::estimate_fee_from_simulation(simulation, 1000)
                .unwrap()
                .gas_fee,
            DEFAULT_GAS_BUDGET
        );
    }

    #[test]
    fn estimate_fee_rejects_failed_simulation() {
        let simulation = SuiSimulation {
            success: false,
            error: Some("Move abort".into()),
            computation_cost: 100,
            storage_cost: 20,
        };

        let error = SuiChain::estimate_fee_from_simulation(simulation, 1000).unwrap_err();
        assert!(matches!(
            error,
            crate::Error::SuiError(SuiError::ExecutionFailed {
                phase: ExecutionPhase::Simulation,
                reason,
            }) if reason == "Move abort"
        ));
    }

    #[test]
    fn estimate_fee_rejects_cost_overflow() {
        let simulation = SuiSimulation {
            success: true,
            error: None,
            computation_cost: u64::MAX,
            storage_cost: 1,
        };

        let error = SuiChain::estimate_fee_from_simulation(simulation, 1000).unwrap_err();
        assert!(matches!(
            error,
            crate::Error::SuiError(SuiError::TransactionBuild { stage: "estimate gas", reason })
                if reason == "gas cost overflow"
        ));
    }

    #[test]
    fn sign_transaction_produces_verifiable_signature() {
        let tx_bytes = transaction_bytes();
        let (key_bytes, private_key) = generated_private_key();

        let signature = SuiChain::sign_transaction(&tx_bytes, private_key).unwrap();
        let signature = sui_sdk_types::UserSignature::from_bytes(&signature).unwrap();
        let transaction = sui_sdk_types::Transaction::from_bcs(&tx_bytes).unwrap();
        Ed25519PrivateKey::new(key_bytes)
            .verifying_key()
            .verify_transaction(&transaction, &signature)
            .unwrap();
    }

    #[test]
    fn sign_transaction_rejects_invalid_private_key_length() {
        let error =
            SuiChain::sign_transaction(&transaction_bytes(), ChainPrivateKey::from("00010203"))
                .unwrap_err();

        assert!(matches!(
            error,
            crate::Error::SignError(message) if message == "Sui private key must be 32 bytes"
        ));
    }

    #[test]
    fn sign_transaction_rejects_invalid_transaction_bcs() {
        let (_, private_key) = generated_private_key();
        let error = SuiChain::sign_transaction(&[0xff], private_key).unwrap_err();

        assert!(matches!(
            error,
            crate::Error::SignError(message) if message.starts_with("invalid Sui transaction BCS:")
        ));
    }

    #[test]
    fn queried_gasless_transaction_preserves_effects_fee_without_dividing_by_zero() {
        let mut tx = SuiQueriedTransaction {
            digest: "digest".into(),
            checkpoint: 42,
            timestamp_seconds: 1_700_000_000,
            success: true,
            net_gas_usage: 0,
            gas_price: 0,
            gas_owner: TEST_ADDRESS.into(),
            balance_changes: vec![],
        };
        assert_eq!(SuiChain::extract_gas_used(&tx), Some((0.0, 0)));
        tx.net_gas_usage = -100;
        assert_eq!(
            SuiChain::extract_gas_used(&tx),
            Some((-100.0 / SUI_VALUE, 0))
        );
        tx.gas_price = 1000;
        tx.net_gas_usage = 2000;
        assert_eq!(
            SuiChain::extract_gas_used(&tx),
            Some((2000.0 / SUI_VALUE, 2))
        );
        tx.gas_price = i64::MAX as u64 + 1;
        assert_eq!(SuiChain::extract_gas_used(&tx), None);
    }
}
