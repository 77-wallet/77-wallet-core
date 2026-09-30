mod base;
mod scroll;

#[cfg(test)]
mod tests;

use super::EthChain;
use alloy::{primitives::U256, rpc::types::TransactionRequest};
use wallet_types::chain::chain::ChainCode;

impl EthChain {
    pub async fn estimate_additional_fee(
        &self,
        tx: TransactionRequest,
    ) -> crate::Result<Option<U256>> {
        match self.chain_code {
            ChainCode::Base => Ok(Some(base::estimate_l1_data_fee(&self.provider, tx).await?)),
            ChainCode::Scroll => Ok(Some(scroll::estimate_l1_data_fee(&self.provider, tx).await?)),
            _ => Ok(None),
        }
    }
}
