use crate::{Error, Result};
use solana_sdk::pubkey::Pubkey;
use wallet_utils::address;

pub const SPL_TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

pub fn token_program_id_from_owner(owner: &str) -> Result<Pubkey> {
    let owner = owner.trim();

    if owner == SPL_TOKEN_PROGRAM_ID {
        return Ok(address::parse_sol_address(SPL_TOKEN_PROGRAM_ID)?);
    }

    if owner == spl_token_2022::id().to_string() {
        return Ok(spl_token_2022::id());
    }

    Err(Error::Other(format!(
        "unsupported sol token program owner: {owner}"
    )))
}

pub async fn resolve_mint_token_program_id(
    provider: &crate::sol::Provider,
    mint: Pubkey,
) -> Result<Pubkey> {
    let mint_account = provider.account_info(mint).await?;
    let mint_account = mint_account
        .value
        .ok_or_else(|| Error::Other(format!("mint account not found: {mint}")))?;

    token_program_id_from_owner(&mint_account.owner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_spl_token_owner_maps_to_legacy_program() {
        let program_id = token_program_id_from_owner(SPL_TOKEN_PROGRAM_ID)
            .expect("legacy token program should resolve");

        assert_eq!(
            program_id,
            address::parse_sol_address(SPL_TOKEN_PROGRAM_ID)
                .expect("legacy token program address should parse")
        );
    }

    #[test]
    fn token_2022_owner_maps_to_token_2022_program() {
        let program_id = token_program_id_from_owner(&spl_token_2022::id().to_string())
            .expect("token-2022 program should resolve");

        assert_eq!(program_id, spl_token_2022::id());
    }

    #[test]
    fn unknown_owner_is_rejected() {
        let err =
            token_program_id_from_owner("unknown-owner").expect_err("unknown owner should fail");

        assert!(
            matches!(err, Error::Other(message) if message.contains("unsupported sol token program owner"))
        );
    }
}
