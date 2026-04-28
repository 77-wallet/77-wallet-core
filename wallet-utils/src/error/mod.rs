pub mod crypto;
pub mod global_value;
pub mod http;
pub mod parse;
pub mod ping;
pub mod serde;
pub mod sign_err;
pub mod snowflake;
pub use snowflake::SnowflakeError;

/// 重试策略枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPolicy {
    Never,
    Delay,
}

/// 检查错误消息是否包含HTML标签的辅助函数
pub fn is_html_error_message(msg: &str) -> bool {
    let msg = msg.to_ascii_lowercase();
    msg.contains("<html") || msg.contains("<!doctype html")
}

/// 错误重试策略trait
/// 为所有错误类型提供统一的重试策略判断方法
pub trait RetryableError {
    /// 判断是否为网络错误
    fn is_network_error(&self) -> bool {
        false
    }

    /// 判断是否为HTML错误页面
    fn is_html_error(&self) -> bool {
        false
    }

    /// 判断是否可以延迟重试
    fn is_delay_retryable(&self) -> bool {
        false
    }

    /// 获取重试策略
    fn retry_policy(&self) -> RetryPolicy {
        if self.is_delay_retryable() {
            RetryPolicy::Delay
        } else {
            RetryPolicy::Never
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Serde error: {0}")]
    Serde(#[from] serde::SerdeError),
    #[error("Parse error: {0}")]
    Parse(#[from] parse::ParseError),
    #[error("Http error: {0}")]
    Http(#[from] http::HttpError),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Parse error: `{0}`")]
    Sign(#[from] sign_err::SignError),
    #[error("Crypto error: `{0}`")]
    Crypto(#[from] crypto::CryptoError),
    #[error("snowflake error: `{0}`")]
    SnowflakeError(#[from] SnowflakeError),
    #[error("Other error: `{0}`")]
    Other(String),
    #[error("Address index overflow occured")]
    AddressIndexOverflowOccured,
    #[error("Icmp error: `{0}`")]
    Icmp(#[from] ping::IcmpError),
    #[error("Global value error: `{0}`")]
    GlobalValue(#[from] global_value::GlobalValueError),
}

impl Error {
    pub fn is_network_error(&self) -> bool {
        matches!(self, Error::Http(_) | Error::Icmp(_))
    }
}

impl RetryableError for Error {
    fn is_network_error(&self) -> bool {
        self.is_network_error()
    }

    fn is_html_error(&self) -> bool {
        false
    }

    fn is_delay_retryable(&self) -> bool {
        self.is_network_error()
    }

    fn retry_policy(&self) -> RetryPolicy {
        if self.is_network_error() {
            RetryPolicy::Delay
        } else {
            RetryPolicy::Never
        }
    }
}
