pub mod address;
pub mod derive;
pub mod error;
pub mod keypair;
pub mod language;
pub mod xpriv;

pub use crate::error::Error;
pub use keypair::KeyPair;

#[cfg(test)]
mod tests {
    use solana_sdk::signer::Signer;

    #[test]
    fn test_solana_keypair_roundtrip() {
        let key = solana_sdk::signature::Keypair::new();
        let pubkey = key.pubkey();
        let encoded = key.to_base58_string();
        let decoded = solana_sdk::signature::Keypair::from_base58_string(&encoded);

        assert_eq!(decoded.pubkey(), pubkey);
        assert!(!encoded.is_empty());
    }
}
