use thiserror::Error;
use tonic::Code;
use wallet_utils::{RetryPolicy, RetryableError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionPhase {
    Simulation,
    Submission,
}

impl std::fmt::Display for ExecutionPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Simulation => formatter.write_str("simulation"),
            Self::Submission => formatter.write_str("submission"),
        }
    }
}

#[derive(Error, Debug)]
pub enum SuiError {
    #[error("invalid Sui {field}: {reason}")]
    InvalidInput { field: &'static str, reason: String },
    #[error("unsupported Sui feature: {feature}")]
    Unsupported { feature: &'static str },
    #[error("Sui {resource} not found: {id}")]
    NotFound { resource: &'static str, id: String },
    #[error("Sui {operation} transport error ({code:?}): {message}")]
    Transport {
        operation: &'static str,
        code: Code,
        message: String,
    },
    #[error("invalid Sui {operation} response field {field}: {reason}")]
    InvalidResponse {
        operation: &'static str,
        field: &'static str,
        reason: String,
    },
    #[error("Sui transaction {phase} failed: {reason}")]
    ExecutionFailed {
        phase: ExecutionPhase,
        reason: String,
    },
    #[error(
        "Sui transaction submission outcome unknown ({code:?}, digest {digest:?}): {message}; query the digest before retrying"
    )]
    SubmissionOutcomeUnknown {
        digest: Option<String>,
        code: Code,
        message: String,
    },
    #[error("Sui transaction build failed at {stage}: {reason}")]
    TransactionBuild { stage: &'static str, reason: String },
    #[error("Sui programmable transaction input count exceeds u16")]
    TooManyProgrammableInputs,
    #[error("Sui programmable transaction command count exceeds u16")]
    TooManyProgrammableCommands,
    #[error("Insufficient balance: only {0} available, but {1} needed")]
    InsufficientBalance(u64, u64),
    #[error("Insufficient fee: only {0} available, but {1} needed")]
    InsufficientFee(u64, u64),
}

impl SuiError {
    pub(crate) fn transport(operation: &'static str, status: tonic::Status) -> Self {
        Self::Transport {
            operation,
            code: status.code(),
            message: status.message().to_string(),
        }
    }

    pub(crate) fn invalid_response(
        operation: &'static str,
        field: &'static str,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidResponse {
            operation,
            field,
            reason: reason.into(),
        }
    }
}

impl RetryableError for SuiError {
    fn is_network_error(&self) -> bool {
        matches!(
            self,
            Self::Transport {
                code: Code::Cancelled
                    | Code::Unknown
                    | Code::DeadlineExceeded
                    | Code::ResourceExhausted
                    | Code::Aborted
                    | Code::Internal
                    | Code::Unavailable,
                ..
            } | Self::SubmissionOutcomeUnknown {
                code: Code::Cancelled
                    | Code::Unknown
                    | Code::DeadlineExceeded
                    | Code::ResourceExhausted
                    | Code::Aborted
                    | Code::Internal
                    | Code::Unavailable,
                ..
            }
        )
    }

    fn is_delay_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport {
                code: Code::Cancelled
                    | Code::Unknown
                    | Code::DeadlineExceeded
                    | Code::ResourceExhausted
                    | Code::Aborted
                    | Code::Internal
                    | Code::Unavailable,
                ..
            }
        )
    }

    fn retry_policy(&self) -> RetryPolicy {
        if self.is_delay_retryable() {
            RetryPolicy::Delay
        } else {
            RetryPolicy::Never
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_transport_errors_preserve_retry_semantics() {
        let transient =
            SuiError::transport("get balance", tonic::Status::unavailable("node restarting"));
        assert!(transient.is_network_error());
        assert_eq!(transient.retry_policy(), RetryPolicy::Delay);

        let rejected =
            SuiError::transport("get balance", tonic::Status::invalid_argument("bad owner"));
        assert!(!rejected.is_network_error());
        assert_eq!(rejected.retry_policy(), RetryPolicy::Never);
    }

    #[test]
    fn unknown_submission_outcome_is_never_automatically_retried() {
        let error = SuiError::SubmissionOutcomeUnknown {
            digest: Some("digest".into()),
            code: Code::DeadlineExceeded,
            message: "timed out".into(),
        };
        assert!(error.is_network_error());
        assert!(!error.is_delay_retryable());
        assert_eq!(error.retry_policy(), RetryPolicy::Never);
    }
}
