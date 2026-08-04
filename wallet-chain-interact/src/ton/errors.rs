use thiserror::Error;
use tonlib_core::{TonAddressParseError, cell::TonCellError, message::TonMessageError};
use wallet_utils::{RetryPolicy, RetryableError};

#[derive(Error, Debug)]
pub enum TonError {
    #[error("run getMethod Resp:{0}")]
    RunGetMethodResp(String),
    #[error("cell build {0}")]
    CellBuild(#[from] TonCellError),
    #[error("ton address {0}")]
    TonAddress(#[from] TonAddressParseError),
    #[error("ton message error {0}")]
    TonMsg(#[from] TonMessageError),
    #[error("{0}")]
    TonNodeError(#[from] wallet_transport::errors::TransportError),
    #[error("{0}")]
    NotTokenParse(String),
    #[error(
        "invalid NFT transfer amount: attached nanotons {attached}, forward nanotons {forward}"
    )]
    InvalidNftAmount { attached: String, forward: String },
    #[error("invalid TON wallet message count: actual {actual}, maximum {max}")]
    InvalidMessageCount { actual: usize, max: usize },
    #[error("TON wallet message/mode count mismatch: messages {messages}, modes {modes}")]
    MessageModeCountMismatch { messages: usize, modes: usize },
    #[error("NFT query ID overflow: base {base}, item index {index}")]
    QueryIdOverflow { base: u64, index: usize },
}

impl RetryableError for TonError {
    fn is_network_error(&self) -> bool {
        match self {
            TonError::TonNodeError(e) => e.is_network_error(),
            _ => false,
        }
    }

    fn is_html_error(&self) -> bool {
        match self {
            TonError::TonNodeError(e) => e.is_html_error(),
            _ => false,
        }
    }

    fn is_delay_retryable(&self) -> bool {
        match self {
            TonError::TonNodeError(e) => e.is_delay_retryable(),
            _ => false,
        }
    }

    fn retry_policy(&self) -> RetryPolicy {
        match self {
            TonError::TonNodeError(e) => e.retry_policy(),
            _ => RetryPolicy::Never,
        }
    }
}
