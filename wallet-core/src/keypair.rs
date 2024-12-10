pub trait KeyPair {
    type Error;

    fn chain_code(&self) -> wallet_types::chain::chain::ChainCode;

    fn generate_with_derivation(
        seed: Vec<u8>,
        derivation_path: &str,
        chain_code: &wallet_types::chain::chain::ChainCode,
        network: wallet_types::chain::network::NetworkKind,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn private_key(&self) -> Result<String, Self::Error>;

    fn pubkey(&self) -> String;

    fn network(&self) -> wallet_types::chain::network::NetworkKind;

    fn private_key_bytes(&self) -> Result<Vec<u8>, Self::Error>;

    fn address(&self) -> String;

    fn derivation_path(&self) -> String;
}
