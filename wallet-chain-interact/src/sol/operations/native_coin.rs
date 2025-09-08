use crate::sol::{Provider, operations::contract::TOKEN_PRAMS_ID};
use alloy::primitives::U256;
use async_trait::async_trait;
use solana_sdk::program_pack::Pack as _;
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token_2022::instruction::{close_account, sync_native};
use wallet_utils::address;

// WRAP SOL program
const WSOL_ADDRESS: &str = "So11111111111111111111111111111111111111112";

// mean W_SOl
// 构建交易指令
// 手续费
// 执行交易

// Deposit SOL into the WSOL token account (similar to ETH -> WETH deposit),
pub struct Deposit<'a> {
    pub from: solana_sdk::pubkey::Pubkey,
    pub amount: u64,
    pub mint_pubkey: solana_sdk::pubkey::Pubkey,
    pub provider: &'a Provider,
}

impl<'a> Deposit<'a> {
    pub fn new_wsol(from: &str, amount: U256, provider: &'a Provider) -> crate::Result<Self> {
        let from = address::parse_sol_address(from)?;
        let mint_pubkey = address::parse_sol_address(WSOL_ADDRESS)?;

        Ok(Self {
            from,
            amount: amount.to::<u64>(),
            mint_pubkey,
            provider,
        })
    }

    pub fn associated_account_instruction(
        &self,
        program_id: solana_sdk::pubkey::Pubkey,
    ) -> solana_sdk::instruction::Instruction {
        create_associated_token_account(&self.from, &self.from, &self.mint_pubkey, &program_id)
    }
}

#[async_trait]
impl super::SolInstructionOperation for Deposit<'_> {
    async fn instructions(&self) -> crate::Result<Vec<solana_sdk::instruction::Instruction>> {
        let account_address = get_associated_token_address(&self.from, &self.mint_pubkey);
        let account = self.provider.account_info(account_address).await?;

        let mut instruction = vec![];
        let program_id = address::parse_sol_address(TOKEN_PRAMS_ID)?;
        // spl账户不存在，添加一个床架账号的指令
        if account.value.is_none() {
            instruction.push(self.associated_account_instruction(program_id));
        }

        // 转移sol 指令
        instruction.push(solana_sdk::system_instruction::transfer(
            &self.from,
            &account_address,
            self.amount,
        ));

        // sync native 指令
        instruction.push(
            sync_native(&program_id, &account_address)
                .map_err(|_e| crate::Error::Other("sync native ".to_string()))?,
        );

        Ok(instruction)
    }
}

#[async_trait]
impl super::SolTransferOperation for Deposit<'_> {
    fn payer(&self) -> crate::Result<solana_sdk::pubkey::Pubkey> {
        Ok(self.from)
    }

    async fn extra_fee(&self) -> crate::Result<Option<u64>> {
        let destination_pubkey = get_associated_token_address(&self.from, &self.mint_pubkey);

        // Check whether the address has a token account.
        let to_account = self.provider.account_info(destination_pubkey).await?;

        if to_account.value.is_none() {
            let data_len = spl_token_2022::state::Account::LEN;
            let value = self
                .provider
                .get_minimum_balance_for_rent(data_len as u64)
                .await?;
            return Ok(Some(value));
        }
        Ok(None)
    }
}

pub struct Withdraw<'a> {
    pub from: solana_sdk::pubkey::Pubkey,
    pub amount: u64,
    pub mint_pubkey: solana_sdk::pubkey::Pubkey,
    pub provider: &'a Provider,
}

impl<'a> Withdraw<'a> {
    pub fn new_wsol(from: &str, amount: U256, provider: &'a Provider) -> crate::Result<Self> {
        let from = address::parse_sol_address(from)?;
        let mint_pubkey = address::parse_sol_address(WSOL_ADDRESS)?;

        Ok(Self {
            from,
            amount: amount.to::<u64>(),
            mint_pubkey,
            provider,
        })
    }
}

#[async_trait]
impl super::SolInstructionOperation for Withdraw<'_> {
    async fn instructions(&self) -> crate::Result<Vec<solana_sdk::instruction::Instruction>> {
        let account_address = get_associated_token_address(&self.from, &self.mint_pubkey);

        let mut instruction = vec![];
        let program_id = address::parse_sol_address(TOKEN_PRAMS_ID)?;

        // sync native 指令
        instruction.push(
            close_account(
                &program_id,
                &account_address,
                &self.from,
                &self.from,
                &vec![&self.from],
            )
            .map_err(|_e| crate::Error::Other("close account instruction error ".to_string()))?,
        );

        Ok(instruction)
    }
}

#[async_trait]
impl super::SolTransferOperation for Withdraw<'_> {
    fn payer(&self) -> crate::Result<solana_sdk::pubkey::Pubkey> {
        Ok(self.from)
    }

    async fn extra_fee(&self) -> crate::Result<Option<u64>> {
        Ok(None)
    }
}
