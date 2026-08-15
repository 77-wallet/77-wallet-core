use dogcoin::hex::DisplayHex;
use ed25519_dalek_bip32::{DerivationPath, ExtendedSecretKey};
use std::str::FromStr;
use tonlib_core::wallet::{mnemonic::KeyPair, ton_wallet::TonWallet};
use wallet_types::chain::{
    address::r#type::{BtcAddressType, TonAddressType},
    chain::ChainCode,
};
pub struct TonKeyPair {
    tron_family: ChainCode,
    private_key: ExtendedSecretKey,
    network: wallet_types::chain::network::NetworkKind,
    derivation: String,
    address_type: TonAddressType,
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct TonInstance {
    pub(crate) chain_code: ChainCode,
    pub network: wallet_types::chain::network::NetworkKind,
    pub address_type: TonAddressType,
}

impl TonInstance {
    pub const TON_DERIVATION_PATH: &'static str = "m/44'/607'/0'";

    pub fn address_from_private_key(
        private_key: &str,
        address_type: TonAddressType,
    ) -> Result<String, crate::Error> {
        let bytes = wallet_utils::hex_func::hex_decode(private_key)?;

        let sk = ed25519_dalek_bip32::SecretKey::from_bytes(&bytes)
            .map_err(|_e| crate::Error::PriKey("ton invalid private key".to_string()))?;

        let pk = ed25519_dalek_bip32::PublicKey::from(&sk);

        let mut sk = sk.as_bytes().to_vec();
        let pk = pk.as_bytes().to_vec();
        sk.extend(&pk);

        let key_pair = KeyPair {
            secret_key: sk,
            public_key: pk,
        };

        let wallet = TonWallet::new(address_type.to_version(), key_pair).map_err(|e| {
            crate::Error::PriKey(format!("ton wallet address generation failed: {e}"))
        })?;
        Ok(wallet.address.to_base64_url())
    }
}

impl TonKeyPair {
    pub(crate) fn generate_with_derivation_and_address_type(
        seed: Vec<u8>,
        derivation_path: &str,
        chain_code: &ChainCode,
        network: wallet_types::chain::network::NetworkKind,
        address_type: TonAddressType,
    ) -> Result<Self, crate::Error> {
        let drive_path = DerivationPath::from_str(derivation_path)
            .map_err(|e| crate::Error::PriKey(format!("ton invalid derivation path: {e:?}")))?;

        let private_key = ExtendedSecretKey::from_seed(&seed)
            .map_err(|e| crate::Error::PriKey(format!("ton invalid seed: {e:?}")))?
            .derive(&drive_path)
            .map_err(|e| crate::Error::PriKey(format!("ton derive failed: {e:?}")))?;

        Ok(Self {
            tron_family: chain_code.to_owned(),
            private_key,
            network,
            derivation: derivation_path.to_owned(),
            address_type,
        })
    }
}

//  获取派生路径
impl wallet_core::derive::GenDerivation for TonInstance {
    type Error = crate::Error;
    fn generate(
        _address_type: &Option<BtcAddressType>,
        input_index: i32,
    ) -> Result<String, crate::Error> {
        let path = if input_index < 0 {
            let i = wallet_utils::address::i32_index_to_unhardened_u32(input_index)?;
            crate::add_index(Self::TON_DERIVATION_PATH, i, true)?
        } else {
            let i = input_index as u32;
            crate::add_index(Self::TON_DERIVATION_PATH, i, true)?
        };
        Ok(path)
    }
}

impl wallet_core::KeyPair for TonKeyPair {
    type Error = crate::Error;

    fn network(&self) -> wallet_types::chain::network::NetworkKind {
        self.network
    }

    // 生成keypair
    fn generate_with_derivation(
        seed: Vec<u8>,
        derivation_path: &str,
        chain_code: &ChainCode,
        network: wallet_types::chain::network::NetworkKind,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        Self::generate_with_derivation_and_address_type(
            seed,
            derivation_path,
            chain_code,
            network,
            TonAddressType::V4R2,
        )
    }

    fn private_key(&self) -> Result<String, Self::Error> {
        // Ok(self.private_key.key.to_lower_hex_string())
        Ok(self.private_key.secret_key.as_bytes().to_lower_hex_string())
    }

    fn pubkey(&self) -> String {
        self.private_key
            .public_key()
            .to_bytes()
            .to_lower_hex_string()
        // wallet_utils::hex_func::hex_encode(&self.private_key.public_key()[1..])
    }

    fn address(&self) -> String {
        let key_pair = KeyPair {
            secret_key: self.private_key.secret_key.as_bytes().to_vec(),
            public_key: self.private_key.public_key().as_bytes().to_vec(),
        };

        let wallet = TonWallet::new(self.address_type.to_version(), key_pair)
            .expect("derived TON keypair must produce a valid wallet");

        let testnet = match self.network {
            wallet_types::chain::network::NetworkKind::Mainnet => false,
            _ => true,
        };
        wallet.address.to_base64_url_flags(true, testnet)
    }

    fn derivation_path(&self) -> String {
        self.derivation.clone()
    }

    fn chain_code(&self) -> ChainCode {
        self.tron_family
    }

    fn private_key_bytes(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.private_key.secret_key.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod test {
    use super::TonInstance;
    use crate::instance::{ChainObject, ton::TonKeyPair};
    use tonlib_core::{
        TonAddress,
        wallet::{mnemonic::KeyPair as TonLibKeyPair, ton_wallet::TonWallet},
    };
    use wallet_core::{KeyPair, derive::GenDerivation, language::Language, xpriv};
    use wallet_types::chain::{
        address::r#type::{AddressType, TonAddressType},
        chain::ChainCode,
        network::NetworkKind,
    };

    const FIXED_TEST_SEED: [u8; 64] = [7; 64];
    const TON_ADDRESS_CASES: [(TonAddressType, &str); 7] = [
        (
            TonAddressType::V2R1,
            "0:2dc70ef65d55aa1f1afb0dff67ba289b700a96240c4cfafe8f4b8bc2ed416f1a",
        ),
        (
            TonAddressType::V2R2,
            "0:200645c2d93fcdc571dfea7d3acf29e3344a7efff989600d8b57fea326b09285",
        ),
        (
            TonAddressType::V3R1,
            "0:461e583f9fa6c295ea1b8640823397e7f32da6ae239afdfe36f20fa39c0a7538",
        ),
        (
            TonAddressType::V3R2,
            "0:427d965f2c200adc281e9341eec3a4dfccee82f3940c37a2b248bf3ab7a0805e",
        ),
        (
            TonAddressType::V4R1,
            "0:e08d3161e0ad5ad946c146e97aea931ceac32ab7847e39ef98db384fb844fd8a",
        ),
        (
            TonAddressType::V4R2,
            "0:4dbc233f4a32d4a652b4e6bb90253b7c2becc6b41228dfd5849228dc999c721d",
        ),
        (
            TonAddressType::V5R1,
            "0:1af2d6979223bc03809b189d68f7f95b7dfa4c3eeae384360cc837a8392d6510",
        ),
    ];

    fn assert_keypair_address_matches_version(keypair: &TonKeyPair, address_type: TonAddressType) {
        let tonlib_keypair = TonLibKeyPair {
            secret_key: keypair.private_key.secret_key.as_bytes().to_vec(),
            public_key: keypair.private_key.public_key().as_bytes().to_vec(),
        };
        let expected = TonWallet::new(address_type.to_version(), tonlib_keypair)
            .unwrap()
            .address;
        let actual = TonAddress::from_base64_url(&keypair.address()).unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_all_ton_wallet_versions_generate_expected_addresses() {
        let derivation_path = TonInstance::generate(&None, 0).unwrap();

        for (address_type, expected_raw_address) in TON_ADDRESS_CASES {
            let keypair = TonKeyPair::generate_with_derivation_and_address_type(
                FIXED_TEST_SEED.to_vec(),
                &derivation_path,
                &ChainCode::Ton,
                NetworkKind::Mainnet,
                address_type,
            )
            .unwrap();

            assert_keypair_address_matches_version(&keypair, address_type);
            let from_private_key = TonInstance::address_from_private_key(
                &keypair.private_key().unwrap(),
                address_type,
            )
            .unwrap();
            assert_eq!(
                TonAddress::from_base64_url(&from_private_key).unwrap(),
                TonAddress::from_base64_url(&keypair.address()).unwrap()
            );
            assert_eq!(
                TonAddress::from_base64_url(&keypair.address())
                    .unwrap()
                    .to_hex(),
                expected_raw_address
            );
        }
    }

    #[test]
    fn test_network_flag_does_not_change_ton_account_id() {
        let derivation_path = TonInstance::generate(&None, 0).unwrap();
        let mainnet = TonKeyPair::generate_with_derivation_and_address_type(
            FIXED_TEST_SEED.to_vec(),
            &derivation_path,
            &ChainCode::Ton,
            NetworkKind::Mainnet,
            TonAddressType::V5R1,
        )
        .unwrap();
        let testnet = TonKeyPair::generate_with_derivation_and_address_type(
            FIXED_TEST_SEED.to_vec(),
            &derivation_path,
            &ChainCode::Ton,
            NetworkKind::Testnet,
            TonAddressType::V5R1,
        )
        .unwrap();

        assert_ne!(mainnet.address(), testnet.address());
        assert_eq!(
            TonAddress::from_base64_url(&mainnet.address()).unwrap(),
            TonAddress::from_base64_url(&testnet.address()).unwrap()
        );
    }

    #[test]
    fn test_gen() {
        let phrase = Language::English.gen_phrase(12).unwrap().join(" ");
        let password = "";

        let xpriv = xpriv::generate_master_key(1, &phrase, password).unwrap();
        let path = TonInstance::generate(&None, 0).unwrap();

        println!("path: {path}");
        let path = path.as_str();
        let chain_code = ChainCode::Bitcoin;
        let keypair = TonKeyPair::generate_with_derivation(
            xpriv.1,
            &path,
            &chain_code,
            wallet_types::chain::network::NetworkKind::Mainnet,
        )
        .unwrap();

        assert!(!keypair.address().is_empty());
    }

    #[test]
    fn test_gen1() {
        let phrase = Language::English.gen_phrase(12).unwrap().join(" ");
        let password = "";

        let xpriv = xpriv::generate_master_key(1, &phrase, password).unwrap();
        let path = TonInstance::generate(&None, 1).unwrap();

        let chain_code = ChainCode::Bitcoin;
        let keypair = TonKeyPair::generate_with_derivation(
            xpriv.1,
            &path,
            &chain_code,
            wallet_types::chain::network::NetworkKind::Mainnet,
        )
        .unwrap();

        assert!(!keypair.address().is_empty());
    }

    #[test]
    fn test_address_format() {
        let address =
            TonAddress::from_base64_url("UQBud2VI5S1IhaPm3OJ7wYUewhBSK7VhfPbnp_0tvvBpx7ze")
                .unwrap();

        println!("可回退地址   {}", address.to_base64_url_flags(false, false));
        println!("不可回退地址 {} ", address.to_base64_url_flags(true, false));
        println!("不可回退地址 {} ", address.to_base64_std());
        println!("16进制地址 {:?} ", address.to_msg_address());
    }

    #[test]
    // ton address generation
    fn test_print_address() {
        let phrase = "";
        let password = "";

        let (_key, seed) =
            wallet_core::xpriv::generate_master_key_without_check(phrase, &password).unwrap();

        let address_type = vec![
            TonAddressType::V2R1,
            TonAddressType::V2R2,
            TonAddressType::V3R1,
            TonAddressType::V3R2,
            TonAddressType::V4R1,
            TonAddressType::V4R2,
            TonAddressType::V5R1,
        ];

        let network = NetworkKind::Mainnet;
        let chain = ChainCode::Ton;
        for address_type in address_type {
            let address_type = AddressType::Ton(address_type);
            let instance = ChainObject::try_from((&chain, &address_type, network)).unwrap();

            let key_pair = instance
                .gen_keypair_with_index_address_type(&seed, 0)
                .unwrap();

            println!(
                "address = {}, address_type: {}, key = {}",
                key_pair.address(),
                address_type,
                key_pair.private_key().unwrap()
            )
        }
    }
}
