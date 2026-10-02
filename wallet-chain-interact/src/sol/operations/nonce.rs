use crate::sol::operations::{SolInstructionOperation, SolTransferOperation};
use crate::{
    sol::{Provider, SolAccountType},
    types::ChainPrivateKey,
};
use async_trait::async_trait;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair, signer::Signer};
use wallet_utils::address;

/// Parameters for withdrawing native SOL from an initialized durable nonce
/// account. The nonce account owns the lamports, the authority authorizes the
/// withdrawal, and the fee payer pays the transaction fee.
pub struct NonceWithdrawOpt {
    pub nonce_account: Pubkey,
    pub authority: Pubkey,
    pub destination: Pubkey,
    pub fee_payer: Pubkey,
    pub lamports: u64,
    pub(crate) additional_signer: Option<Keypair>,
}

struct NonceSigners {
    fee_payer: Pubkey,
    additional_signer: Option<Keypair>,
}

impl NonceWithdrawOpt {
    /// Check the nonce and keys, then prepare signatures for exec_transaction.
    /// Failed checks leave the existing fee payer and signers unchanged.
    pub async fn prepare(
        &mut self,
        provider: &Provider,
        authority_key: &ChainPrivateKey,
        nonce_key: Option<&ChainPrivateKey>,
    ) -> crate::Result<()> {
        self.validate_account(provider).await?;
        let signers = self.prepare_signers(authority_key, nonce_key)?;

        self.fee_payer = signers.fee_payer;
        self.additional_signer = signers.additional_signer;
        Ok(())
    }

    async fn validate_account(&self, provider: &Provider) -> crate::Result<()> {
        let account = provider.account_info(self.nonce_account).await?;
        let account_type = SolAccountType::from_account_info(account.value.as_ref())?;
        match account_type {
            SolAccountType::InitializedNonceAccount { authority, .. } => {
                if authority != self.authority.to_string() {
                    return Err(crate::Error::TransferError(format!(
                        "nonce authority mismatch: expected {authority}, received {}",
                        self.authority
                    )));
                }
                Ok(())
            }
            SolAccountType::UninitializedNonceAccount { .. } => {
                return Err(crate::Error::TransferError(
                    "nonce account is not initialized".to_string(),
                ));
            }
            SolAccountType::NotFound => {
                return Err(crate::SolError::AccountNotFound(format!(
                    "nonce account {} was not found",
                    self.nonce_account
                ))
                .into());
            }
            _ => {
                return Err(crate::Error::TransferError(format!(
                    "account {} is not an initialized nonce account",
                    self.nonce_account
                ))
                .into());
            }
        }
    }

    fn prepare_signers(
        &self,
        authority_key: &ChainPrivateKey,
        nonce_key: Option<&ChainPrivateKey>,
    ) -> crate::Result<NonceSigners> {
        let decode_key = |key: &ChainPrivateKey| -> crate::Result<Keypair> {
            let bytes = solana_sdk::bs58::decode(&**key).into_vec().map_err(|e| {
                wallet_utils::Error::Parse(wallet_utils::ParseError::SolanaSignatureError(
                    e.to_string(),
                ))
            })?;
            Ok(wallet_utils::parse_func::sol_keypair_from_bytes(&bytes)?)
        };
        let authority_keypair = decode_key(authority_key)?;
        if authority_keypair.pubkey() != self.authority {
            return Err(crate::SolError::SignError(format!(
                "nonce authority private key resolves to {}, expected {}",
                authority_keypair.pubkey(),
                self.authority
            ))
            .into());
        }

        let fee_payer_keypair = nonce_key.map(decode_key).transpose()?;
        if let Some(keypair) = &fee_payer_keypair {
            if keypair.pubkey() != self.nonce_account {
                return Err(crate::SolError::SignError(format!(
                    "fee payer private key resolves to {}, expected {}",
                    keypair.pubkey(),
                    self.nonce_account
                ))
                .into());
            }
        }
        let fee_payer = if fee_payer_keypair.is_some() {
            self.nonce_account
        } else {
            self.authority
        };

        let additional_signer = (fee_payer != self.authority).then_some(authority_keypair);
        Ok(NonceSigners {
            fee_payer,
            additional_signer,
        })
    }

    pub fn new(
        nonce_account: &str,
        authority: &str,
        destination: &str,
        value: &str,
    ) -> crate::Result<Self> {
        let lamports = wallet_utils::unit::convert_to_u256(value, crate::sol::consts::SOL_DECIMAL)?
            .to::<u64>();

        Ok(Self {
            nonce_account: address::parse_sol_address(nonce_account)?,
            authority: address::parse_sol_address(authority)?,
            destination: address::parse_sol_address(destination)?,
            fee_payer: address::parse_sol_address(authority)?,
            lamports,
            additional_signer: None,
        })
    }

    pub fn from_lamports(
        nonce_account: Pubkey,
        authority: Pubkey,
        destination: Pubkey,
        lamports: u64,
        fee_payer: Pubkey,
    ) -> Self {
        Self {
            nonce_account,
            authority,
            destination,
            fee_payer,
            lamports,
            additional_signer: None,
        }
    }
}

#[async_trait]
impl SolInstructionOperation for NonceWithdrawOpt {
    async fn instructions(&self) -> crate::Result<Vec<Instruction>> {
        Ok(vec![
            solana_sdk::system_instruction::withdraw_nonce_account(
                &self.nonce_account,
                &self.authority,
                &self.destination,
                self.lamports,
            ),
        ])
    }
}

#[async_trait]
impl SolTransferOperation for NonceWithdrawOpt {
    fn other_keypair(&self) -> Vec<Keypair> {
        self.additional_signer
            .iter()
            .map(Keypair::insecure_clone)
            .collect()
    }

    fn payer(&self) -> crate::Result<Pubkey> {
        Ok(self.fee_payer)
    }

    async fn extra_fee(&self) -> crate::Result<Option<u64>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::system_instruction::SystemInstruction;

    #[test]
    fn prepares_payer_roles_without_mutating_operation() {
        let authority = Keypair::new();
        let nonce = Keypair::new();
        let authority_key = ChainPrivateKey::from(authority.to_base58_string());
        let nonce_key = ChainPrivateKey::from(nonce.to_base58_string());
        let params = NonceWithdrawOpt::from_lamports(
            nonce.pubkey(),
            authority.pubkey(),
            Pubkey::new_unique(),
            100,
            authority.pubkey(),
        );
        let default = params.prepare_signers(&authority_key, None).unwrap();
        assert_eq!(default.fee_payer, authority.pubkey());
        assert!(default.additional_signer.is_none());
        let nonce_paid = params
            .prepare_signers(&authority_key, Some(&nonce_key))
            .unwrap();
        assert_eq!(nonce_paid.fee_payer, nonce.pubkey());
        assert_eq!(
            nonce_paid.additional_signer.unwrap().pubkey(),
            authority.pubkey()
        );
        assert!(matches!(
            params.prepare_signers(&authority_key, Some(&authority_key)),
            Err(crate::Error::SolError(crate::SolError::SignError(_)))
        ));
        assert!(matches!(
            params.prepare_signers(&ChainPrivateKey::from("invalid-key"), None),
            Err(crate::Error::UtilsError(_))
        ));
        assert_eq!(params.fee_payer, authority.pubkey());
        assert!(params.additional_signer.is_none());
    }

    #[test]
    fn failed_signer_preparation_keeps_existing_signer_state() {
        let nonce = Keypair::new();
        let authority = Keypair::new();
        let mut params = NonceWithdrawOpt::from_lamports(
            nonce.pubkey(),
            authority.pubkey(),
            Pubkey::new_unique(),
            0,
            nonce.pubkey(),
        );
        params.additional_signer = Some(authority.insecure_clone());
        let key = ChainPrivateKey::from(authority.to_base58_string());
        assert!(matches!(
            params.prepare_signers(&key, Some(&key)),
            Err(crate::Error::SolError(crate::SolError::SignError(_)))
        ));
        assert_eq!(params.fee_payer, nonce.pubkey());
        assert_eq!(
            params.additional_signer.unwrap().pubkey(),
            authority.pubkey()
        );
    }

    #[tokio::test]
    async fn nonce_paid_withdrawal_has_two_valid_signatures() {
        use solana_sdk::{
            hash::Hash, signature::Keypair, signer::Signer, transaction::Transaction,
        };
        let nonce = Keypair::new();
        let authority = Keypair::new();
        let mut params = NonceWithdrawOpt::from_lamports(
            nonce.pubkey(),
            authority.pubkey(),
            Pubkey::new_unique(),
            100,
            nonce.pubkey(),
        );
        params.additional_signer = Some(authority);
        let instructions = params.instructions().await.unwrap();
        let other = params.other_keypair();
        let mut signers: Vec<&Keypair> = other.iter().collect();
        signers.push(&nonce);
        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&params.payer().unwrap()),
            &signers,
            Hash::new_unique(),
        );
        assert_eq!(tx.signatures.len(), 2);
        assert!(tx.verify().is_ok());
        let mut unsigned = Transaction::new_unsigned(tx.message.clone());
        assert!(
            unsigned
                .try_sign(&[&other[0]], tx.message.recent_blockhash)
                .is_err()
        );
    }

    #[tokio::test]
    async fn authority_paid_withdrawal_needs_only_primary_signer() {
        use solana_sdk::{
            hash::Hash, signature::Keypair, signer::Signer, transaction::Transaction,
        };
        let authority = Keypair::new();
        let params = NonceWithdrawOpt::new(
            &Pubkey::new_unique().to_string(),
            &authority.pubkey().to_string(),
            &Pubkey::new_unique().to_string(),
            "0.001",
        )
        .unwrap();
        assert!(params.other_keypair().is_empty());
        assert_eq!(params.payer().unwrap(), authority.pubkey());
        let tx = Transaction::new_signed_with_payer(
            &params.instructions().await.unwrap(),
            Some(&params.payer().unwrap()),
            &[&authority],
            Hash::new_unique(),
        );
        assert_eq!(tx.signatures.len(), 1);
        assert!(tx.verify().is_ok());
    }

    #[tokio::test]
    async fn builds_nonce_withdraw_instruction_with_distinct_roles() {
        let nonce_account = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let destination = Pubkey::new_unique();
        let fee_payer = Pubkey::new_unique();
        let params = NonceWithdrawOpt::from_lamports(
            nonce_account,
            authority,
            destination,
            123_456,
            fee_payer,
        );

        let instructions = params.instructions().await.unwrap();
        let instruction = instructions.first().unwrap();
        let decoded: SystemInstruction = bincode::deserialize(&instruction.data).unwrap();

        assert_eq!(params.payer().unwrap(), fee_payer);
        assert_eq!(decoded, SystemInstruction::WithdrawNonceAccount(123_456));
        assert_eq!(instruction.accounts[0].pubkey, nonce_account);
        assert!(!instruction.accounts[0].is_signer);
        assert_eq!(instruction.accounts[1].pubkey, destination);
        assert_eq!(instruction.accounts[4].pubkey, authority);
        assert!(instruction.accounts[4].is_signer);
    }
}
