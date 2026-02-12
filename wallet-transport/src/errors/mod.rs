use thiserror::Error;
use wallet_utils::{RetryPolicy, RetryableError};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("node response: {}", .0)]
    NodeResponseError(NodeResponseError),
    #[error("query result empty")]
    EmptyResult,
    #[error("Utils error: {0}")]
    Utils(#[from] wallet_utils::error::Error),
    #[error("Rumqttc v5 option error: {0}")]
    RumqttcV5Option(#[from] rumqttc::v5::OptionError),
}

impl std::fmt::Display for NodeResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node response error: code={}, rpc={}, message={:?}",
            self.code, self.rpc, self.message
        )
    }
}
impl TransportError {
    pub fn is_network_error(&self) -> bool {
        match self {
            TransportError::Utils(e) => e.is_network_error(),
            _ => false,
        }
    }

    pub fn retry_policy(&self) -> RetryPolicy {
        match self {
            TransportError::NodeResponseError(e) => {
                if e.is_delay_retryable() {
                    RetryPolicy::Delay
                } else {
                    RetryPolicy::Never
                }
            }

            TransportError::Utils(e) => {
                if e.is_network_error() {
                    RetryPolicy::Delay
                } else {
                    RetryPolicy::Never
                }
            }

            TransportError::EmptyResult => RetryPolicy::Never,

            TransportError::RumqttcV5Option(_) => RetryPolicy::Never,
        }
    }
}

impl RetryableError for TransportError {
    fn is_network_error(&self) -> bool {
        match self {
            TransportError::Utils(e) => e.is_network_error(),
            _ => false,
        }
    }

    fn is_html_error(&self) -> bool {
        match self {
            TransportError::NodeResponseError(node_response_error) => {
                node_response_error.is_html_error()
            }
            TransportError::Utils(error) => error.is_html_error(),
            _ => false,
        }
    }

    fn is_delay_retryable(&self) -> bool {
        self.retry_policy() == RetryPolicy::Delay
    }

    fn retry_policy(&self) -> RetryPolicy {
        match self {
            TransportError::NodeResponseError(e) => {
                if e.is_delay_retryable() {
                    RetryPolicy::Delay
                } else {
                    RetryPolicy::Never
                }
            }

            TransportError::Utils(e) => {
                if e.is_network_error() {
                    RetryPolicy::Delay
                } else {
                    RetryPolicy::Never
                }
            }

            TransportError::EmptyResult => RetryPolicy::Never,

            TransportError::RumqttcV5Option(_) => RetryPolicy::Never,
        }
    }
}

#[derive(Debug)]
pub struct NodeResponseError {
    pub code: i64,
    pub rpc: String,
    pub message: Option<String>,
}

impl NodeResponseError {
    pub fn new(code: i64, rpc: &str, message: Option<String>) -> Self {
        Self {
            code,
            rpc: rpc.to_string(),
            message,
        }
    }

    pub fn is_network_error(&self) -> bool {
        self.code == 503
    }

    pub fn is_delay_retryable(&self) -> bool {
        matches!(self.code, 502 | 503 | 504) || self.is_html_error()
    }

    fn is_html_error(&self) -> bool {
        self.message
            .as_deref()
            .map(|m| wallet_utils::error::is_html_error_message(m))
            .unwrap_or(false)
    }
}

impl RetryableError for NodeResponseError {
    fn is_network_error(&self) -> bool {
        self.is_network_error()
    }

    fn is_html_error(&self) -> bool {
        self.is_html_error()
    }

    fn is_delay_retryable(&self) -> bool {
        self.is_delay_retryable()
    }

    fn retry_policy(&self) -> RetryPolicy {
        if self.is_delay_retryable() {
            RetryPolicy::Delay
        } else {
            RetryPolicy::Never
        }
    }
}
