use serde::Deserialize;
use solana_sdk::{
    nonce::state::{State as NonceState, Versions as NonceVersions},
    system_program,
};

/// Semantic classification of a Solana account.
///
/// A durable nonce account is owned by the System Program just like a normal
/// wallet, so callers must inspect its state data rather than relying on the
/// owner alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolAccountType {
    NotFound,
    SystemAccount {
        lamports: u64,
    },
    InitializedNonceAccount {
        lamports: u64,
        authority: String,
        lamports_per_signature: u64,
    },
    UninitializedNonceAccount {
        lamports: u64,
    },
    Other {
        lamports: u64,
        owner: String,
        space: u64,
    },
}

impl SolAccountType {
    pub fn from_account_info(account: Option<&AccountInfo>) -> crate::Result<Self> {
        let Some(account) = account else {
            return Ok(Self::NotFound);
        };

        if account.owner != system_program::ID.to_string() {
            return Ok(Self::other(account));
        }

        if account.space == 0 && account.data.first().is_none_or(String::is_empty) {
            return Ok(Self::SystemAccount {
                lamports: account.lamports,
            });
        }

        if account.space != NonceState::size() as u64 {
            return Ok(Self::other(account));
        }

        let encoded = account
            .data
            .first()
            .ok_or_else(|| crate::ParseErr::Serialize("missing nonce account data".to_string()))?;
        let versions = wallet_utils::hex_func::bs64_decode::<NonceVersions>(encoded)?;

        Ok(match versions.state() {
            NonceState::Initialized(data) => Self::InitializedNonceAccount {
                lamports: account.lamports,
                authority: data.authority.to_string(),
                lamports_per_signature: data.get_lamports_per_signature(),
            },
            NonceState::Uninitialized => Self::UninitializedNonceAccount {
                lamports: account.lamports,
            },
        })
    }

    fn other(account: &AccountInfo) -> Self {
        Self::Other {
            lamports: account.lamports,
            owner: account.owner.clone(),
            space: account.space,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AccountInfo {
    pub data: Vec<String>,
    pub executable: bool,
    pub lamports: u64,
    pub owner: String,
    #[serde(rename = "rentEpoch")]
    pub rent_epoch: u64,
    pub space: u64,
}

#[derive(Debug, Deserialize)]
pub struct Balance {
    pub context: Context,
    pub value: u128,
}

#[derive(Debug, Deserialize)]
pub struct Context {
    pub slot: u128,
}

#[derive(Debug, Deserialize)]
pub struct TokenAccount {
    pub context: Context,
    pub value: Vec<TokenAccountValue>,
}
impl TokenAccount {
    pub fn balance(&self) -> String {
        if self.value.is_empty() {
            return "0".to_string();
        }
        self.value
            .first()
            .unwrap()
            .account
            .data
            .parsed
            .info
            .token_amount
            .amount
            .clone()
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenAccountValue {
    pub account: Account,
    pub pubkey: String,
}

#[derive(Debug, Deserialize)]
pub struct Account {
    pub data: AccountData,
    pub executable: bool,
    pub lamports: u64,
    pub owner: String,
    #[serde(rename = "rentEpoch")]
    pub rent_epoch: u64,
    pub space: u64,
}

#[derive(Debug, Deserialize)]
pub struct AccountData {
    pub parsed: Parsed,
    pub program: String,
    pub space: u64,
}

#[derive(Debug, Deserialize)]
pub struct Parsed {
    pub info: Info,
    #[serde(rename = "type")]
    pub types: String,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    #[serde(rename = "isNative")]
    pub is_native: bool,
    pub mint: String,
    pub owner: String,
    pub state: String,
    #[serde(rename = "tokenAmount")]
    pub token_amount: TokenAmount,
}

#[derive(Debug, Deserialize)]
pub struct TokenAmount {
    pub amount: String,
    pub decimals: u8,
    #[serde(rename = "uiAmount")]
    pub ui_amount: f64,
    #[serde(rename = "uiAmountString")]
    pub ui_amount_string: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{hash::Hash, nonce::state::DurableNonce, pubkey::Pubkey};

    fn account(owner: String, data: Vec<String>, lamports: u64, space: u64) -> AccountInfo {
        AccountInfo {
            data,
            executable: false,
            lamports,
            owner,
            rent_epoch: 0,
            space,
        }
    }

    #[test]
    fn classifies_missing_and_plain_system_accounts() {
        assert_eq!(
            SolAccountType::from_account_info(None).unwrap(),
            SolAccountType::NotFound
        );

        let account = account(system_program::ID.to_string(), vec![String::new()], 42, 0);
        assert_eq!(
            SolAccountType::from_account_info(Some(&account)).unwrap(),
            SolAccountType::SystemAccount { lamports: 42 }
        );
    }

    #[test]
    fn classifies_initialized_nonce_and_exposes_authority() {
        let authority = Pubkey::new_unique();
        let state = NonceState::new_initialized(
            &authority,
            DurableNonce::from_blockhash(&Hash::new_unique()),
            5_000,
        );
        let encoded = wallet_utils::hex_func::bs64_encode(&NonceVersions::new(state)).unwrap();
        let account = account(
            system_program::ID.to_string(),
            vec![encoded, "base64".to_string()],
            123,
            NonceState::size() as u64,
        );

        assert_eq!(
            SolAccountType::from_account_info(Some(&account)).unwrap(),
            SolAccountType::InitializedNonceAccount {
                lamports: 123,
                authority: authority.to_string(),
                lamports_per_signature: 5_000,
            }
        );
    }

    #[test]
    fn classifies_uninitialized_nonce() {
        let mut bytes = bincode::serialize(&NonceVersions::new(NonceState::Uninitialized)).unwrap();
        bytes.resize(NonceState::size(), 0);
        let encoded = wallet_utils::bytes_to_base64(&bytes);
        let account = account(
            system_program::ID.to_string(),
            vec![encoded, "base64".to_string()],
            456,
            NonceState::size() as u64,
        );

        assert_eq!(
            SolAccountType::from_account_info(Some(&account)).unwrap(),
            SolAccountType::UninitializedNonceAccount { lamports: 456 }
        );
    }

    #[test]
    fn does_not_misclassify_other_or_malformed_accounts() {
        let other_owner = Pubkey::new_unique().to_string();
        let other = account(other_owner.clone(), Vec::new(), 1, 165);
        assert_eq!(
            SolAccountType::from_account_info(Some(&other)).unwrap(),
            SolAccountType::Other {
                lamports: 1,
                owner: other_owner,
                space: 165,
            }
        );

        let malformed = account(
            system_program::ID.to_string(),
            vec!["not-base64".to_string()],
            2,
            NonceState::size() as u64,
        );
        assert!(matches!(
            SolAccountType::from_account_info(Some(&malformed)),
            Err(crate::Error::UtilsError(_))
        ));
        let missing_data = account(
            system_program::ID.to_string(),
            vec![],
            2,
            NonceState::size() as u64,
        );
        assert!(matches!(
            SolAccountType::from_account_info(Some(&missing_data)),
            Err(crate::Error::ParseError(crate::ParseErr::Serialize(_)))
        ));
        let corrupt_state = account(
            system_program::ID.to_string(),
            vec![
                wallet_utils::bytes_to_base64(&[255; 80]),
                "base64".to_string(),
            ],
            2,
            80,
        );
        assert!(matches!(
            SolAccountType::from_account_info(Some(&corrupt_state)),
            Err(crate::Error::UtilsError(_))
        ));
    }
}
