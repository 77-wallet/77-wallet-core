use crate::sol::{
    Provider,
    operations::{SolInstructionOperation, SolTransferOperation},
};
use solana_sdk::{bpf_loader_upgradeable, signature::Keypair};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use wallet_utils::address;

//  token param id
pub const TOKEN_PRAMS_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const META_PRAMS_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

pub struct TokenTransferBuild<'a> {
    pub program_id: solana_sdk::pubkey::Pubkey,
    pub params: &'a super::transfer::TransferOpt<'a>,
    pub authority_pubkey: solana_sdk::pubkey::Pubkey,
    pub mint_pubkey: solana_sdk::pubkey::Pubkey,
}
impl<'a> TokenTransferBuild<'a> {
    pub fn new(
        params: &'a super::transfer::TransferOpt,
        mint_pubkey: solana_sdk::pubkey::Pubkey,
    ) -> crate::Result<Self> {
        let program_id = address::parse_sol_address(TOKEN_PRAMS_ID)?;

        Ok(Self {
            program_id,
            params,
            authority_pubkey: params.from,
            mint_pubkey,
        })
    }
}

impl<'a> TokenTransferBuild<'a> {
    // token transfer instruction
    pub async fn transfer_instruction(
        &self,
    ) -> crate::Result<Vec<solana_sdk::instruction::Instruction>> {
        let mut instruction = vec![];

        let source_pubkey = get_associated_token_address(&self.params.from, &self.mint_pubkey);
        let destination_pubkey = get_associated_token_address(&self.params.to, &self.mint_pubkey);

        // Check whether the address has a token account.
        let to_account = self
            .params
            .provider
            .account_info(destination_pubkey)
            .await?;
        if to_account.value.is_none() {
            instruction.push(self.associated_account_instruction());
        }

        let transfer = spl_token_2022::instruction::transfer_checked(
            &self.program_id,
            &source_pubkey,
            &self.mint_pubkey,
            &destination_pubkey,
            &self.authority_pubkey,
            &[],
            self.params.value,
            self.params.decimal,
        )
        .map_err(|e| crate::Error::Other(format!("build transfer instruction error:{}", e)))?;
        instruction.push(transfer);

        Ok(instruction)
    }

    // associated token account instruction
    pub fn associated_account_instruction(&self) -> solana_sdk::instruction::Instruction {
        create_associated_token_account(
            &self.authority_pubkey,
            &self.params.to,
            &self.mint_pubkey,
            &self.program_id,
        )
    }
}

pub struct UpdateAuth<'a> {
    pub current_authority: solana_sdk::pubkey::Pubkey,
    pub new_authority: solana_sdk::pubkey::Pubkey,
    pub program_address: solana_sdk::pubkey::Pubkey,
    pub new_auth_key: String,
    pub provider: &'a Provider,
}

impl<'a> UpdateAuth<'a> {
    pub fn new(
        current_authority: &str,
        new_authority: &str,
        program_address: &str,
        keypair: &str,
        provider: &'a Provider,
    ) -> crate::Result<Self> {
        Ok(Self {
            current_authority: wallet_utils::address::parse_sol_address(current_authority)?,
            new_authority: wallet_utils::address::parse_sol_address(new_authority)?,
            program_address: wallet_utils::address::parse_sol_address(program_address)?,
            new_auth_key: keypair.to_string(),
            provider,
        })
    }
}

#[async_trait::async_trait]
impl SolInstructionOperation for UpdateAuth<'_> {
    async fn instructions(
        &self,
    ) -> Result<Vec<solana_sdk::instruction::Instruction>, crate::Error> {
        let instructions = bpf_loader_upgradeable::set_upgrade_authority_checked(
            &self.program_address,
            &self.current_authority,
            &self.new_authority,
        );

        Ok(vec![instructions])
    }
}

#[async_trait::async_trait]
impl SolTransferOperation for UpdateAuth<'_> {
    fn other_keypair(&self) -> Vec<solana_sdk::signature::Keypair> {
        vec![Keypair::from_base58_string(&self.new_auth_key)]
    }

    fn payer(&self) -> crate::Result<solana_sdk::pubkey::Pubkey> {
        Ok(self.current_authority)
    }

    async fn extra_fee(&self) -> crate::Result<Option<u64>> {
        Ok(None)
    }
}
