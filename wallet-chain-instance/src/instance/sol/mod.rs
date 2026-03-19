pub mod address;
use solana_sdk::signer::Signer;
use wallet_core::{KeyPair, derive::Derive};
use wallet_types::chain::{address::r#type::BtcAddressType, chain::ChainCode};

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct SolanaInstance {
    pub(crate) chain_code: ChainCode,
    pub network: wallet_types::chain::network::NetworkKind,
}

impl wallet_core::derive::GenDerivation for SolanaInstance {
    type Error = crate::Error;
    fn generate(
        _address_type: &Option<BtcAddressType>,
        input_index: i32,
    ) -> Result<String, crate::Error> {
        let index = wallet_utils::address::i32_index_to_unhardened_u32(input_index)?;
        let path = crate::add_solana_index(wallet_types::constant::SOLANA_DERIVATION_PATH, index)?;
        Ok(path)
    }
}

impl Derive for SolanaInstance {
    type Error = crate::Error;
    type Item = SolanaKeyPair;

    fn derive_with_derivation_path(
        &self,
        seed: Vec<u8>,
        derivation_path: &str,
    ) -> Result<Self::Item, Self::Error> {
        SolanaKeyPair::generate_with_derivation(
            seed,
            derivation_path,
            &self.chain_code,
            self.network,
        )
    }
}

pub struct SolanaKeyPair {
    solana_family: ChainCode,
    keypair: solana_sdk::signature::Keypair,
    pubkey: String,
    derivation: String,
    network: wallet_types::chain::network::NetworkKind,
}

impl KeyPair for SolanaKeyPair {
    type Error = crate::Error;

    fn generate_with_derivation(
        seed: Vec<u8>,
        derivation_path: &str,
        chain_code: &ChainCode,
        network: wallet_types::chain::network::NetworkKind,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let derivation =
            solana_sdk::derivation_path::DerivationPath::from_absolute_path_str(derivation_path)
                .map_err(|e| crate::Error::Keypair(crate::KeypairError::Solana(e.to_string())))?;
        let keypair =
            solana_sdk::signature::keypair_from_seed_and_derivation_path(&seed, Some(derivation))
                .map_err(|e| crate::Error::Keypair(crate::KeypairError::Solana(e.to_string())))?;

        let pubkey = keypair.pubkey().to_string();

        Ok(Self {
            solana_family: chain_code.to_owned(),
            pubkey,
            keypair,
            derivation: derivation_path.to_string(),
            network,
        })
    }

    fn network(&self) -> wallet_types::chain::network::NetworkKind {
        self.network
    }

    fn private_key(&self) -> Result<String, Self::Error> {
        // solana_sdk::derivation_path::DerivationPath
        Ok(self.keypair.to_base58_string())
    }
    fn pubkey(&self) -> String {
        self.pubkey.clone()
    }

    fn address(&self) -> String {
        self.keypair.pubkey().to_string()
    }

    fn derivation_path(&self) -> String {
        self.derivation.clone()
    }

    fn chain_code(&self) -> wallet_types::chain::chain::ChainCode {
        self.solana_family
    }

    fn private_key_bytes(&self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.keypair.to_bytes().to_vec())
        // Ok().map_err(|e| crate::Error::Parse(e.into()))?)
    }
}

pub fn secret_key_to_address(pkey: &[u8]) -> Result<solana_sdk::pubkey::Pubkey, crate::Error> {
    let keypair = solana_sdk::signer::keypair::Keypair::from_bytes(pkey).unwrap();
    Ok(keypair.pubkey())
}

pub fn address_from_secret_key(prik: &str) -> Result<solana_sdk::pubkey::Pubkey, crate::Error> {
    let keypair = solana_sdk::signer::keypair::Keypair::from_base58_string(prik);
    Ok(keypair.pubkey())
}

#[cfg(test)]
mod tests {
    use super::*;
    use coins_bip39::{English, Mnemonic};
    use crate::instance::sol::address::SolGenAddress;
    use wallet_core::language::Language;
    use wallet_core::{address::GenAddress, derive::GenDerivation, KeyPair};
    use wallet_types::chain::{chain::ChainCode, network::NetworkKind};

    fn test_instance() -> SolanaInstance {
        SolanaInstance {
            chain_code: ChainCode::Solana,
            network: NetworkKind::Mainnet,
        }
    }

    fn test_seed() -> Vec<u8> {
        let mnemonic = Language::English.gen_phrase(12).unwrap().join(" ");
        let mnemonic =
            Mnemonic::<English>::new_from_phrase(&mnemonic).expect("Invalid mnemonic phrase");
        mnemonic.to_seed(Some("")).unwrap().to_vec()
    }

    #[test]
    fn test_generate_derivation_path() {
        assert_eq!(
            SolanaInstance::generate(&None, 0).unwrap(),
            "m/44'/501'/0'/0"
        );
        assert_eq!(
            SolanaInstance::generate(&None, 7).unwrap(),
            "m/44'/501'/7'/0"
        );
    }

    #[test]
    fn test_generate_keypair_and_address() {
        let seed = test_seed();
        let instance = test_instance();
        let derivation_path = SolanaInstance::generate(&None, 2147483647).unwrap();
        assert_eq!(derivation_path, "m/44'/501'/2147483647'/0");

        let keypair = instance
            .derive_with_derivation_path(seed, &derivation_path)
            .unwrap();

        assert_eq!(keypair.chain_code(), ChainCode::Solana);
        assert_eq!(keypair.network(), NetworkKind::Mainnet);
        assert_eq!(keypair.derivation_path(), derivation_path);
        assert_eq!(keypair.address(), keypair.pubkey());

        let private_key = keypair.private_key().unwrap();
        assert_eq!(
            address_from_secret_key(&private_key).unwrap().to_string(),
            keypair.address()
        );

        assert_eq!(
            secret_key_to_address(&keypair.private_key_bytes().unwrap())
                .unwrap()
                .to_string(),
            keypair.address()
        );
    }

    #[test]
    fn test_gen_address_matches_generated_keypair() {
        let seed = test_seed();
        let instance = test_instance();
        let derivation_path = SolanaInstance::generate(&None, 0).unwrap();
        let keypair = instance
            .derive_with_derivation_path(seed, &derivation_path)
            .unwrap();
        let private_key_bytes = keypair.private_key_bytes().unwrap();

        let generated = SolGenAddress {}
            .generate(&private_key_bytes)
            .unwrap();

        assert_eq!(generated.to_string(), keypair.address());
        assert_eq!((SolGenAddress {}).chain_code(), &ChainCode::Solana);
    }
}
