use super::{
    operations::multisig::program::{MultisigArgs, ProgramConfig},
    protocol::{
        account::{AccountInfo, Balance, TokenAccount},
        block::Prioritization,
        contract::TotalSupply,
        transaction::{CommitmentConfig, Status},
    },
};
use crate::sol::protocol::{
    Response,
    block::{Block, BlockHash},
    transaction::{SignatureStatus, SimValue, SimulateTransactionConfig, TransactionResponse},
};
use serde_json::json;
use solana_sdk::{
    address_lookup_table::{AddressLookupTableAccount, state::AddressLookupTable},
    hash::Hash,
    instruction::Instruction,
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    transaction::{Transaction, VersionedTransaction},
};
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;
use wallet_transport::{client::RpcClient, types::JsonRpcParams};

pub struct Provider {
    pub client: RpcClient,
}

#[derive(Debug, Default)]
pub struct SendTransactionOpts {
    pub preflight_commitment: Option<CommitmentConfig>,
    pub max_retries: Option<u32>,
}

impl SendTransactionOpts {
    pub fn legacy_broadcast() -> Self {
        Self {
            preflight_commitment: Some(CommitmentConfig::Processed),
            max_retries: Some(5),
        }
    }

    pub fn legacy_send_only() -> Self {
        Self {
            preflight_commitment: Some(CommitmentConfig::Processed),
            max_retries: Some(0),
        }
    }
}

impl Provider {
    pub fn new(rpc_client: RpcClient) -> crate::Result<Self> {
        Ok(Self { client: rpc_client })
    }

    pub async fn balance(&self, address: &str) -> crate::Result<Balance> {
        let params = JsonRpcParams::default()
            .method("getBalance")
            .params(vec![address]);

        Ok(self.client.invoke_request::<_, Balance>(params).await?)
    }

    pub async fn token_balance(&self, token: &str, address: &str) -> crate::Result<TokenAccount> {
        let req = vec![
            address.into(),
            json!({
                "mint": token,
            }),
            json!({
                "encoding": "jsonParsed"
            }),
        ];

        let params = JsonRpcParams::default()
            .method("getTokenAccountsByOwner")
            .params(req);

        Ok(self
            .client
            .invoke_request::<_, TokenAccount>(params)
            .await?)
    }

    pub async fn token_symbol(&self, mint: &str) -> crate::Result<String> {
        Ok(self.token_metadata(mint).await?.1)
    }

    pub async fn token_name(&self, mint: &str) -> crate::Result<String> {
        Ok(self.token_metadata(mint).await?.0)
    }

    /// Resolve (name, symbol) using the mint's metadata pointer when present.
    /// Only absent extensions fall back to Metaplex; corrupt data and RPC
    /// failures must remain errors rather than silently selecting other data.
    async fn token_metadata(&self, mint: &str) -> crate::Result<(String, String)> {
        use spl_token_2022::{
            extension::{
                BaseStateWithExtensions, ExtensionType, StateWithExtensions,
                metadata_pointer::MetadataPointer,
            },
            state::Mint,
        };
        use spl_token_metadata_interface::state::TokenMetadata;

        let mint = Pubkey::from_str(mint).map_err(|e| crate::Error::Other(e.to_string()))?;
        let mint_account = self
            .account_info(mint)
            .await?
            .value
            .ok_or_else(|| crate::Error::Other(format!("mint account not found: {mint}")))?;
        let token_program =
            super::operations::token_program::token_program_id_from_owner(&mint_account.owner)?;
        let mint_data = Self::metadata_account_data(&mint_account)?;
        let state = StateWithExtensions::<Mint>::unpack(&mint_data)
            .map_err(|e| crate::Error::Other(format!("Invalid mint account: {e}")))?;
        let metadata_program =
            wallet_utils::address::parse_sol_address(super::operations::contract::META_PRAMS_ID)?;
        let metadata_pda = Pubkey::find_program_address(
            &[b"metadata", metadata_program.as_ref(), mint.as_ref()],
            &metadata_program,
        )
        .0;

        if token_program == spl_token_2022::id() {
            let extensions = state
                .get_extension_types()
                .map_err(|e| crate::Error::Other(format!("Invalid mint extensions: {e}")))?;
            if extensions.contains(&ExtensionType::MetadataPointer) {
                let pointer = state
                    .get_extension::<MetadataPointer>()
                    .map_err(|e| crate::Error::Other(format!("Invalid metadata pointer: {e}")))?;
                if let Some(address) = Option::<Pubkey>::from(pointer.metadata_address) {
                    if address == mint {
                        let metadata = state
                            .get_variable_len_extension::<TokenMetadata>()
                            .map_err(|e| {
                                crate::Error::Other(format!("Invalid Token-2022 metadata: {e}"))
                            })?;
                        if metadata.mint != mint {
                            return Err(crate::Error::Other(
                                "Token-2022 metadata mint mismatch".into(),
                            ));
                        }
                        return Ok((metadata.name, metadata.symbol));
                    }
                    // Metaplex accounts have a known owner, canonical address and
                    // format. Other metadata programs need their own decoder.
                    if address != metadata_pda {
                        return Err(crate::Error::Other(format!(
                            "Unsupported external metadata pointer: {address}"
                        )));
                    }
                }
            }
        }

        let account = self
            .account_info(metadata_pda)
            .await?
            .value
            .ok_or_else(|| crate::Error::Other("Metadata account not found".into()))?;
        if account.owner != metadata_program.to_string() {
            return Err(crate::Error::Other(
                "Invalid Metaplex metadata owner".into(),
            ));
        }
        let data = Self::metadata_account_data(&account)?;
        let metadata = mpl_token_metadata::accounts::Metadata::from_bytes(&data)
            .map_err(|e| crate::Error::Other(format!("Invalid Metaplex metadata: {e}")))?;
        if metadata.key != mpl_token_metadata::types::Key::MetadataV1
            || metadata.mint.to_bytes() != mint.to_bytes()
        {
            return Err(crate::Error::Other(
                "Invalid Metaplex metadata type or mint".into(),
            ));
        }
        Ok((metadata.name, metadata.symbol))
    }

    fn metadata_account_data(account: &AccountInfo) -> crate::Result<Vec<u8>> {
        if account.data.get(1).map(String::as_str) != Some("base64") {
            return Err(crate::Error::Other(
                "Expected base64 metadata account data".into(),
            ));
        }
        let encoded = account
            .data
            .first()
            .ok_or_else(|| crate::Error::Other("Empty metadata account".into()))?;
        Ok(wallet_utils::base64_to_bytes(encoded)?)
    }

    pub async fn get_transaction_index(&self, multisig_pda: &Pubkey) -> crate::Result<u64> {
        let account = self
            .account_info(*multisig_pda)
            .await?
            .value
            .ok_or(crate::Error::Other(
                "not found multisig account".to_string(),
            ))?;

        let multisig = account.data.first().unwrap();
        let multisig_pda = MultisigArgs::from_str(multisig)?;

        Ok(multisig_pda.stale_transaction_index)
    }

    pub async fn get_config_program(&self, config_pda: &Pubkey) -> crate::Result<ProgramConfig> {
        let account = self
            .account_info(*config_pda)
            .await?
            .value
            .ok_or(crate::Error::Other("not found config account".to_string()))?;

        let config = account.data.first().unwrap();
        let program_config = ProgramConfig::from_str(config)?;

        Ok(program_config)
    }

    pub async fn latest_block(
        &self,
        commitment: CommitmentConfig,
    ) -> crate::Result<Response<BlockHash>> {
        let params = JsonRpcParams::default()
            .method("getLatestBlockhash")
            .params(vec![json!({
                "commitment": commitment.to_string()
            })]);

        Ok(self
            .client
            .invoke_request::<_, Response<BlockHash>>(params)
            .await?)
    }

    pub async fn latest_blockhash(&self, commitment: CommitmentConfig) -> crate::Result<Hash> {
        let block = self.latest_block(commitment).await?;

        let hash = Hash::from_str(&block.value.blockhash)
            .map_err(|e| crate::Error::Other(e.to_string()))?;

        Ok(hash)
    }

    // execute transaction
    pub async fn execute_transaction(
        &self,
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        keypair: &[&Keypair],
    ) -> crate::Result<String> {
        let block_hash = self.latest_blockhash(CommitmentConfig::Finalized).await?;

        let tx =
            Transaction::new_signed_with_payer(&instructions, Some(payer), keypair, block_hash);

        let raw_tx =
            solana_sdk::bs58::encode(wallet_utils::hex_func::bin_encode_bytes(&tx)?).into_string();

        self.broadcast_legacy(&raw_tx).await
    }

    pub async fn build_legacy_signed_tx(
        &self,
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        keypair: &[&Keypair],
    ) -> crate::Result<(String, String)> {
        let block_hash = self.latest_blockhash(CommitmentConfig::Processed).await?;

        let tx =
            Transaction::new_signed_with_payer(&instructions, Some(payer), keypair, block_hash);

        // raw_tx(base58)
        let raw_tx =
            solana_sdk::bs58::encode(wallet_utils::hex_func::bin_encode_bytes(&tx)?).into_string();

        // hash = Signature
        let tx_hash = tx.signatures[0].to_string();

        Ok((tx_hash, raw_tx))
    }

    pub async fn broadcast_legacy(&self, raw_tx: &str) -> crate::Result<String> {
        let result = self
            .send_transaction_with_opts(raw_tx, SendTransactionOpts::legacy_broadcast())
            .await?;
        Ok(result)
    }

    // 执行v0的交易,
    pub async fn execute_v0_transaction(
        &self,
        instructions: Vec<Instruction>,
        alts: Vec<AddressLookupTableAccount>,
        payer: &Pubkey,
        keypair: &[&Keypair],
    ) -> crate::Result<String> {
        let raw_tx = self
            .build_vo_transaction(&instructions, alts, payer, Some(keypair))
            .await?;

        self.send_bs64_tx(&raw_tx).await
    }

    async fn build_vo_transaction(
        &self,
        instructions: &[Instruction],
        alts: Vec<AddressLookupTableAccount>,
        payer: &Pubkey,
        keypairs: Option<&[&Keypair]>,
    ) -> crate::Result<String> {
        let recent_blockhash = self.latest_blockhash(CommitmentConfig::Finalized).await?;

        let message = solana_program::message::v0::Message::try_compile(
            payer,
            instructions,
            &alts,
            recent_blockhash,
        )
        .map_err(crate::SolError::InstructionCompile)?;

        let message = VersionedMessage::V0(message);

        let tx = if let Some(keypairs) = keypairs {
            VersionedTransaction::try_new(message, keypairs).map_err(|e| {
                crate::SolError::SignError(format!("failed to sign transaction {e}"))
            })?
        } else {
            let signature = vec![Signature::default()];
            VersionedTransaction {
                signatures: signature,
                message,
            }
        };

        let raw_tx = wallet_utils::bytes_to_base64(&wallet_utils::hex_func::bin_encode_bytes(&tx)?);

        Ok(raw_tx)
    }

    // 发送 and 等待确认交易
    pub async fn send_and_confirm_transaction(
        &self,
        instructions: Vec<Instruction>,
        payer: &Pubkey,
        keypair: &[&Keypair],
        retries: usize,
    ) -> crate::Result<String> {
        let get_status_time = 600;

        for _ in 0..retries {
            // 执行交易
            let block_hash = self.latest_blockhash(CommitmentConfig::Processed).await?;

            let block_hash_str = block_hash.to_string();
            let tx =
                Transaction::new_signed_with_payer(&instructions, Some(payer), keypair, block_hash);
            let raw_tx = solana_sdk::bs58::encode(wallet_utils::hex_func::bin_encode_bytes(&tx)?)
                .into_string();

            let tx_hash = self
                .send_transaction_with_opts(&raw_tx, SendTransactionOpts::legacy_send_only())
                .await?;

            // query result
            for _ in 0..get_status_time {
                sleep(Duration::from_millis(500)).await;

                match self.get_signature_status(&tx_hash).await? {
                    Some(res) => match res.status {
                        Status::Ok(_) => {
                            // 验证确认数量
                            if res.confirmation_status == CommitmentConfig::Confirmed.to_string() {
                                return Ok(tx_hash);
                            }
                        }
                        Status::Err(e) => {
                            let error_msg = wallet_utils::serde_func::serde_to_string(&e)?;
                            return Err(crate::Error::TransferError(error_msg));
                        }
                    },
                    None => {
                        // 验证blockhash有效
                        if !self
                            .is_blockhash_vaild(&block_hash_str, CommitmentConfig::Processed)
                            .await?
                        {
                            if self
                                .query_transaction(
                                    &tx_hash,
                                    CommitmentConfig::Finalized.to_string(),
                                )
                                .await
                                .is_ok()
                            {
                                return Ok(tx_hash);
                            } else {
                                // 交易查询失败，跳出内层循环，准备重试发送交易
                                break;
                            }
                        }
                    }
                }
            }

            if self
                .query_transaction(&tx_hash, CommitmentConfig::Finalized.to_string())
                .await
                .is_ok()
            {
                return Ok(tx_hash);
            }
        }

        Err(crate::Error::TransferError(format!(
            "failed to transfer and retry {}",
            retries
        )))
    }

    pub async fn get_signature_status(
        &self,
        tx_hash: &str,
    ) -> crate::Result<Option<SignatureStatus>> {
        let params = JsonRpcParams::default()
            .method("getSignatureStatuses")
            .params(vec![vec![tx_hash]]);

        let result = self
            .client
            .invoke_request::<_, Response<Vec<Option<SignatureStatus>>>>(params)
            .await?;

        Ok(result.value[0].clone())
    }

    fn build_send_transaction_request(tx: &str, opts: &SendTransactionOpts) -> serde_json::Value {
        let mut request = vec![json!(tx)];
        let mut config = serde_json::Map::new();

        if let Some(max_retries) = opts.max_retries {
            config.insert("maxRetries".to_string(), json!(max_retries));
        }
        if let Some(preflight_commitment) = &opts.preflight_commitment {
            config.insert(
                "preflightCommitment".to_string(),
                json!(preflight_commitment.to_string()),
            );
        }

        if !config.is_empty() {
            request.push(serde_json::Value::Object(config));
        }

        json!(request)
    }

    pub async fn send_transaction_with_opts(
        &self,
        tx: &str,
        opts: SendTransactionOpts,
    ) -> crate::Result<String> {
        let req = Self::build_send_transaction_request(tx, &opts);
        let params = JsonRpcParams::default()
            .method("sendTransaction")
            .params(req);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn send_transaction(&self, tx: &str, node_retry: bool) -> crate::Result<String> {
        let opts = if node_retry {
            SendTransactionOpts::legacy_broadcast()
        } else {
            SendTransactionOpts::legacy_send_only()
        };

        self.send_transaction_with_opts(tx, opts).await
    }

    // 发送base64编码的消息
    pub async fn send_bs64_tx(&self, tx: &str) -> crate::Result<String> {
        let req = json!([
            tx,
            json!({
                "encoding": "base64",
            })
        ]);

        let params = JsonRpcParams::default()
            .method("sendTransaction")
            .params(req);

        Ok(self.client.invoke_request::<_, String>(params).await?)
    }

    pub async fn is_blockhash_vaild(
        &self,
        blockhash: &str,
        commitment: CommitmentConfig,
    ) -> crate::Result<bool> {
        let req = json!([
            blockhash,
            json!({
                "commitment":commitment.to_string(),
            })
        ]);

        let params = JsonRpcParams::default()
            .method("isBlockhashValid")
            .params(req);

        let result = self
            .client
            .invoke_request::<_, Response<bool>>(params)
            .await?;
        Ok(result.value)
    }

    pub async fn get_recent_prioritization(
        &self,
        account: Option<String>,
    ) -> crate::Result<Prioritization> {
        let account = account.map(|v| vec![vec![v]]);

        let params = JsonRpcParams::default()
            .method("getRecentPrioritizationFees")
            .params(account);

        Ok(self
            .client
            .invoke_request::<_, Prioritization>(params)
            .await?)
    }

    // Not test
    pub async fn simulate_transaction(
        &self,
        instructions: &[Instruction],
        payer: &Pubkey,
    ) -> crate::Result<Response<SimValue>> {
        let blockhash = self.latest_blockhash(CommitmentConfig::Processed).await?;

        let message = Message::new_with_blockhash(instructions, Some(payer), &blockhash);

        let tx = Transaction::new_unsigned(message);
        let config = SimulateTransactionConfig::default();

        let raw_tx = wallet_utils::bytes_to_base64(&wallet_utils::hex_func::bin_encode_bytes(&tx)?);
        let params = JsonRpcParams::default()
            .method("simulateTransaction")
            .params(vec![json!(raw_tx), json!(config)]);

        Ok(self
            .client
            .invoke_request::<_, Response<SimValue>>(params)
            .await?)
    }

    pub async fn simulate_v0_transaction(
        &self,
        instructions: &[Instruction],
        payer: &Pubkey,
        alts: Vec<AddressLookupTableAccount>,
    ) -> crate::Result<Response<SimValue>> {
        let raw_tx = self
            .build_vo_transaction(instructions, alts, payer, None)
            .await?;

        let config = SimulateTransactionConfig::default();

        let params = JsonRpcParams::default()
            .method("simulateTransaction")
            .params(vec![json!(raw_tx), json!(config)]);

        Ok(self
            .client
            .invoke_request::<_, Response<SimValue>>(params)
            .await?)
    }

    pub async fn message_fee(&self, message: &str) -> crate::Result<Response<u64>> {
        let commitment = json!({
            "commitment": "finalized"
        });

        let params = JsonRpcParams::default()
            .method("getFeeForMessage")
            .params(vec![message.into(), commitment]);

        Ok(self
            .client
            .invoke_request::<_, Response<u64>>(params)
            .await?)
    }

    pub async fn query_transaction(
        &self,
        txid: &str,
        commitment: &str,
    ) -> crate::Result<TransactionResponse> {
        let params = JsonRpcParams::default()
            .method("getTransaction")
            .params(json!([
                txid,
                json!({
                    "encoding": "json",
                    "maxSupportedTransactionVersion":1,
                    "rewards": false,
                    commitment:commitment
                }),
            ]));

        Ok(self
            .client
            .invoke_request::<_, TransactionResponse>(params)
            .await?)
    }

    pub async fn get_block(&self, slot: u64) -> crate::Result<Block> {
        let req = json!([
            slot,
            json!({
                "encoding": "json",
                "maxSupportedTransactionVersion":1,
                "rewards": false,
            }),
        ]);
        let params = JsonRpcParams::default().method("getBlock").params(req);

        Ok(self.client.invoke_request::<_, Block>(params).await?)
    }

    pub async fn get_block_height(&self) -> crate::Result<u64> {
        let params: JsonRpcParams<()> = JsonRpcParams::default()
            .method("getBlockHeight")
            .no_params();

        Ok(self.client.invoke_request::<_, u64>(params).await?)
    }

    pub async fn get_slot(&self) -> crate::Result<u64> {
        let params: JsonRpcParams<()> = JsonRpcParams::default().method("getSlot").no_params();

        Ok(self.client.invoke_request::<_, u64>(params).await?)
    }

    pub async fn total_supply(&self, token_addr: &str) -> crate::Result<Response<TotalSupply>> {
        let params = JsonRpcParams::default()
            .method("getTokenSupply")
            .params(vec![token_addr]);

        Ok(self.client.invoke_request(params).await?)
    }

    pub async fn account_info(&self, addr: Pubkey) -> crate::Result<Response<Option<AccountInfo>>> {
        let params = JsonRpcParams::default()
            .method("getAccountInfo")
            .params(vec![
                addr.to_string().into(),
                json!({ "encoding": "base64" }),
            ]);

        Ok(self
            .client
            .invoke_request::<_, Response<Option<AccountInfo>>>(params)
            .await?)
    }

    pub async fn get_minimum_balance_for_rent(&self, data_len: u64) -> crate::Result<u64> {
        let params = JsonRpcParams::default()
            .method("getMinimumBalanceForRentExemption")
            .params(vec![data_len]);

        Ok(self.client.invoke_request(params).await?)
    }

    // 获取多个账号数据
    pub async fn get_multiple_accounts(
        &self,
        addrs: &[String],
    ) -> crate::Result<Vec<Option<AccountInfo>>> {
        let params = JsonRpcParams::default()
            .method("getMultipleAccounts")
            .params(vec![addrs]);

        let result = self
            .client
            .invoke_request::<_, Response<Vec<Option<AccountInfo>>>>(params)
            .await?;

        Ok(result.value)
    }

    // 账号转换为地址表
    pub fn account_to_address_table(
        &self,
        alts: Vec<String>,
        accounts: Vec<Option<AccountInfo>>,
    ) -> crate::Result<Vec<AddressLookupTableAccount>> {
        let mut result = vec![];

        for (pk, acct_opt) in alts.iter().zip(accounts.into_iter()) {
            let acct = match acct_opt {
                Some(a) => a,
                None => {
                    return Err(crate::SolError::AccountNotFound(format!(
                        "ALT {} not found (null from getMultipleAccounts)",
                        pk
                    )))?;
                }
            };

            let b64 = acct.data.get(0).ok_or_else(|| {
                crate::SolError::AccountNotFound(format!("account {} missing data[0]", pk))
            })?;

            let raw = wallet_utils::base64_to_bytes(b64)?;
            let table = AddressLookupTable::deserialize(&raw).map_err(|e| {
                crate::SolError::AddressLookupTable(format!("ALT {} deserialize failed: {e:?}", pk))
            })?;

            result.push(AddressLookupTableAccount {
                key: wallet_utils::address::parse_sol_address(&pk)?,
                addresses: table.addresses.to_vec(),
            });
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::{CommitmentConfig, Provider, SendTransactionOpts};
    use serde_json::json;

    #[test]
    fn build_send_transaction_request_keeps_processed_commitment_and_retries() {
        let req = Provider::build_send_transaction_request(
            "base58-tx",
            &SendTransactionOpts {
                preflight_commitment: Some(CommitmentConfig::Processed),
                max_retries: Some(2),
            },
        );

        assert_eq!(
            req,
            json!([
                "base58-tx",
                {
                    "maxRetries": 2,
                    "preflightCommitment": "processed"
                }
            ])
        );
    }

    #[test]
    fn build_send_transaction_request_omits_config_when_empty() {
        let req =
            Provider::build_send_transaction_request("base58-tx", &SendTransactionOpts::default());

        assert_eq!(req, json!(["base58-tx"]));
    }
}
