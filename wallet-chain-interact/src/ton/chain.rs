use super::{
    consts::DEFAULT_SIGN_KEY,
    errors::TonError,
    get_keypair,
    params::EstimateFeeParams,
    protocol::{
        common::{RunGetMethodParams, StackItem},
        jettons::{JettonMasterResp, JettonWalletAddress, JettonWalletResp},
        transaction::EstimateFeeResp,
    },
    provider::Provider,
};
use crate::{QueryTransactionResult, types::ChainPrivateKey};
use alloy::primitives::U256;
use tonlib_core::{
    cell::{BagOfCells, Cell},
    tlb_types::tlb::TLB as _,
    wallet::{ton_wallet::TonWallet, version_helper::VersionHelper},
};
use wallet_types::chain::address::r#type::TonAddressType;

pub struct TonChain {
    pub provider: Provider,
}

fn wallet_init_data(wallet: &TonWallet) -> Result<Cell, TonError> {
    VersionHelper::get_data(wallet.version, &wallet.key_pair, wallet.wallet_id)
        .map_err(TonError::CellBuild)
}
impl TonChain {
    pub fn new(provider: Provider) -> crate::Result<Self> {
        Ok(Self { provider })
    }
}

#[cfg(test)]
mod tests {
    use super::wallet_init_data;
    use tonlib_core::{
        tlb_types::tlb::TLB,
        wallet::{
            mnemonic::KeyPair,
            ton_wallet::TonWallet,
            versioned::{DEFAULT_WALLET_ID_V5R1, v5::WalletDataV5},
            wallet_version::WalletVersion,
        },
    };

    #[test]
    fn test_v5_wallet_init_data_uses_v5_wallet_id() {
        let key_pair = KeyPair {
            public_key: vec![0; 32],
            secret_key: vec![0; 64],
        };
        let wallet = TonWallet::new(WalletVersion::V5R1, key_pair).unwrap();
        let data = wallet_init_data(&wallet).unwrap();
        let data = WalletDataV5::from_cell(&data).unwrap();

        assert_eq!(wallet.wallet_id, DEFAULT_WALLET_ID_V5R1);
        assert_eq!(data.wallet_id, DEFAULT_WALLET_ID_V5R1);
    }
}

impl TonChain {
    pub async fn balance(&self, addr: &str, token: Option<String>) -> crate::Result<U256> {
        if let Some(t) = token {
            let jetton_address =
                JettonWalletAddress::wallet_address(&t, addr, &self.provider).await?;

            let balance = self
                .provider
                .token_data::<JettonWalletResp>(&jetton_address.to_base64_url())
                .await
                .map_or("0".to_string(), |result| result.balance.to_string());

            Ok(wallet_utils::unit::u256_from_str(&balance)?)
        } else {
            self.provider.balance(addr).await
        }
    }

    pub async fn estimate_fee(
        &self,
        msg_cell: Cell,
        address: &str,
        address_type: TonAddressType,
    ) -> crate::Result<EstimateFeeResp> {
        let version = address_type.to_version();
        let key_pair = get_keypair(DEFAULT_SIGN_KEY.into())?;

        let wallet = TonWallet::new(version, key_pair.clone()).map_err(TonError::CellBuild)?;

        let signed_body = wallet
            .sign_external_body(&msg_cell)
            .map_err(TonError::CellBuild)?;

        // 知道钱包的状态,决定是否部署钱包
        let account_info = self.provider.address_information(&address).await?;

        let boc = BagOfCells::from_root(signed_body)
            .serialize(false)
            .map_err(TonError::CellBuild)?;
        let body = wallet_utils::bytes_to_base64(&boc);

        let mut params = EstimateFeeParams::new(&address, body, true);
        if !account_info.is_init() {
            let code = VersionHelper::get_code(version)
                .map_err(TonError::CellBuild)?
                .clone();
            let data = wallet_init_data(&wallet)?;

            params.init_code = Some(code.to_boc_b64(true).map_err(TonError::CellBuild)?);
            params.init_data = Some(data.to_boc_b64(true).map_err(TonError::CellBuild)?);
        }

        self.provider.estimate_fee(params).await
    }

    // exec transaction
    pub async fn exec(
        &self,
        msg_cell: Cell,
        key: ChainPrivateKey,
        address_type: TonAddressType,
    ) -> crate::Result<String> {
        let key_pair = get_keypair(key)?;

        let version = address_type.to_version();
        let wallet = TonWallet::new(version, key_pair).map_err(TonError::CellBuild)?;

        // 知道钱包的状态,决定是否部署钱包
        let address = wallet.address.to_base64_url();
        let account_info = self.provider.address_information(&address).await?;

        let signed_body = wallet
            .sign_external_body(&msg_cell)
            .map_err(TonError::CellBuild)?;
        let wrapped = wallet
            .wrap_signed_body(signed_body, !account_info.is_init())
            .map_err(TonError::TonMsg)?;

        let boc = BagOfCells::from_root(wrapped);
        let tx = boc.serialize(true).map_err(TonError::CellBuild)?;

        let boc_str = wallet_utils::bytes_to_base64(&tx);
        self.provider.send_boc_return(boc_str).await
    }

    pub async fn decimals(&self, address: &str) -> crate::Result<u8> {
        let result = self
            .provider
            .token_data::<JettonMasterResp>(address)
            .await?;

        result.decimal()
    }

    /// Checks the issuer-controlled status stored in a stablecoin Jetton wallet.
    ///
    /// This is an extension implemented by TON's centralized stablecoin
    /// contract, not part of the generic TEP-74 Jetton interface.
    pub async fn black_address(&self, jetton_master: &str, owner: &str) -> crate::Result<bool> {
        let jetton_wallet =
            JettonWalletAddress::wallet_address(jetton_master, owner, &self.provider).await?;
        let jetton_wallet = jetton_wallet.to_base64_url();

        let account = self.provider.address_information(&jetton_wallet).await?;
        if !jetton_wallet_is_active(&account.state)? {
            return Ok(false);
        }

        let params = RunGetMethodParams::<()>::new(&jetton_wallet, "get_status", vec![]);
        let response = match self.provider.run_get_method(params).await {
            Ok(response) => response,
            Err(error) if get_status_is_unsupported(&error) => {
                return Err(crate::Error::NotSupportApi(
                    "TON Jetton wallet does not implement get_status".to_string(),
                ));
            }
            Err(error) => return Err(error.into()),
        };

        if response.exit_code != 0 {
            return Err(crate::Error::NotSupportApi(format!(
                "TON Jetton wallet get_status exited with code {}",
                response.exit_code
            )));
        }

        let Some(StackItem::Num(_, value)) = response.stack.first() else {
            return Err(crate::Error::Other(
                "TON Jetton wallet get_status returned no numeric status".to_string(),
            ));
        };
        let status = parse_jetton_wallet_status(value)?;

        // bit 0 blocks outgoing transfers; bit 1 blocks incoming transfers.
        Ok(status & 0b11 != 0)
    }

    pub async fn query_tx_res(&self, _hash: &str) -> crate::Result<Option<QueryTransactionResult>> {
        // 这个需要 新的api 支持
        Ok(None)
    }

    pub async fn token_symbol(&self, token: &str) -> crate::Result<String> {
        let token_data = self.provider.token_data::<JettonMasterResp>(token).await?;

        if let Some(symbol) = token_data.jetton_content.data.symbol {
            Ok(symbol)
        } else {
            if let Some(uri) = token_data.jetton_content.data.uri {
                let meta = self.provider.get_token_meta(&uri).await?;

                Ok(meta.symbol)
            } else {
                Ok("".to_string())
            }
        }
    }

    pub async fn token_name(&self, token: &str) -> crate::Result<String> {
        let token_data = self.provider.token_data::<JettonMasterResp>(token).await?;

        if let Some(name) = token_data.jetton_content.data.name {
            Ok(name)
        } else {
            if let Some(uri) = token_data.jetton_content.data.uri {
                let meta = self.provider.get_token_meta(&uri).await?;

                Ok(meta.name)
            } else {
                Ok("".to_string())
            }
        }
    }

    pub async fn block_num(&self) -> crate::Result<u64> {
        let block = self.provider.consensus_block().await?;

        Ok(block.consensus_block)
    }
}

fn get_status_is_unsupported(error: &wallet_transport::TransportError) -> bool {
    let wallet_transport::TransportError::NodeResponseError(error) = error else {
        return false;
    };
    let message = error
        .message
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    message.contains("exit_code: 11")
        || message.contains("exit code: 11")
        || message.contains("method not found")
        || message.contains("unknown method")
}

fn parse_jetton_wallet_status(value: &str) -> crate::Result<u8> {
    let value = value.trim();
    let status = if let Some(value) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u8::from_str_radix(value, 16)
    } else {
        value.parse::<u8>()
    };

    status.map_err(|error| {
        crate::Error::Other(format!(
            "invalid TON Jetton wallet status {value:?}: {error}"
        ))
    })
}

fn jetton_wallet_is_active(state: &str) -> crate::Result<bool> {
    match state.trim().to_ascii_lowercase().as_str() {
        "active" => Ok(true),
        // Jetton wallets are deployed lazily. These states have no readable
        // per-wallet issuer status, so the owner has not been locked.
        "uninitialized" | "uninit" | "nonexist" | "non-existent" => Ok(false),
        // A protocol-level frozen account retains only its StateInit hash. Its
        // data, including the issuer-controlled status, cannot be inspected.
        "frozen" => Err(crate::Error::Other(
            "TON Jetton wallet account is frozen; issuer lock status cannot be determined"
                .to_string(),
        )),
        state => Err(crate::Error::Other(format!(
            "unknown TON Jetton wallet account state: {state}"
        ))),
    }
}

#[cfg(test)]
mod black_address_tests {
    use super::{jetton_wallet_is_active, parse_jetton_wallet_status};

    #[test]
    fn parses_hex_and_decimal_jetton_wallet_status() {
        assert_eq!(parse_jetton_wallet_status("0x3").unwrap(), 3);
        assert_eq!(parse_jetton_wallet_status("2").unwrap(), 2);
    }

    #[test]
    fn distinguishes_uninitialized_and_frozen_jetton_wallets() {
        assert!(jetton_wallet_is_active("active").unwrap());
        assert!(!jetton_wallet_is_active("uninitialized").unwrap());
        assert!(!jetton_wallet_is_active("nonexist").unwrap());

        let error = jetton_wallet_is_active("frozen").unwrap_err();
        assert!(matches!(&error, crate::Error::Other(_)));
        assert!(error.to_string().contains("cannot be determined"));
    }

    #[test]
    fn rejects_unknown_jetton_wallet_state() {
        let error = jetton_wallet_is_active("unexpected").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown TON Jetton wallet account state")
        );
    }
}
