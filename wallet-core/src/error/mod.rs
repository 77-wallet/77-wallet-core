use wallet_utils::RetryableError;

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum Error {
    #[error("Unknown language")]
    UnknownLanguage,
    #[error("Unknown query mode")]
    UnknownQueryMode,
    #[error("Unknown chain code")]
    UnknownChainCode,
    #[error("Unknown coin type: {0}")]
    UnknownCoinType(u32),
    #[error("Mnemonic: {0}")]
    Mnemonic(String),
}

impl From<coins_bip39::MnemonicError> for Error {
    fn from(value: coins_bip39::MnemonicError) -> Self {
        let msg = match value {
            coins_bip39::MnemonicError::InvalidPhrase(_) => "the phrase is invalid".to_string(),
            coins_bip39::MnemonicError::WordlistError(
                _e @ coins_bip39::WordlistError::InvalidWord(_),
            ) => "the word is invalid".to_string(),
            _ => value.to_string(),
        };
        Error::Mnemonic(msg)
    }
}

impl RetryableError for Error {
    // 使用默认实现
    // fn is_network_error(&self) -> bool {
    //     false
    // }

    // 使用默认实现
    // fn is_html_error(&self) -> bool {
    //     false
    // }

    // 使用默认实现
    // fn is_delay_retryable(&self) -> bool {
    //     false
    // }

    // 使用默认实现
    // fn retry_policy(&self) -> RetryPolicy {
    //     RetryPolicy::Never
    // }
}
