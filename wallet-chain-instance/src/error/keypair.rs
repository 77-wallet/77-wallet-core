#[derive(Debug, thiserror::Error)]
pub enum KeypairError {
    #[error("Solana error: `{0}`")]
    Solana(String),
    #[error("Libsecp256k1 error: `{0}`")]
    Libsecp256k1(#[from] libsecp256k1::Error),
}

#[cfg(test)]
mod tests {
    use super::KeypairError;

    #[test]
    fn solana_error_displays_expected_message() {
        let error = KeypairError::Solana("invalid account key".to_string());

        assert_eq!(error.to_string(), "Solana error: `invalid account key`");
    }

    #[test]
    fn libsecp256k1_error_is_converted_into_keypair_error() {
        let libsecp_err = libsecp256k1::SecretKey::parse_slice(&[1_u8; 31]).unwrap_err();
        let error: KeypairError = libsecp_err.into();

        assert!(matches!(error, KeypairError::Libsecp256k1(_)));
        assert!(error.to_string().starts_with("Libsecp256k1 error: `"));
    }
}
