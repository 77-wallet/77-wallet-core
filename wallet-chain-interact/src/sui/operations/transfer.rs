use crate::sui::Provider;
use crate::sui::builder::{ProgrammableTransactionBuilder, SelectCoinHelper};
use crate::sui::consts::SUI_NATIVE_COIN;
use crate::sui::protocol::EstimateFeeResp;
use alloy::primitives::U256;
use sui_sdk_types::{
    Address, Command, GasPayment, ProgrammableTransaction, Transaction, TransactionExpiration,
    TransactionKind, TransferObjects,
};
use wallet_utils::address;

#[derive(Debug)]
pub struct TransferOpt {
    pub from: Address,
    pub to: Address,
    pub amount: u64,
    pub token: Option<String>,
}

impl TransferOpt {
    pub fn new(from: &str, to: &str, amount: U256, token: Option<String>) -> crate::Result<Self> {
        let amount =
            u64::try_from(amount).map_err(|_| crate::sui::error::SuiError::InvalidInput {
                field: "amount",
                reason: "exceeds u64".into(),
            })?;

        Ok(Self {
            from: address::parse_sui_address(from)?,
            to: address::parse_sui_address(to)?,
            amount,
            token,
        })
    }

    pub fn get_coin_type(&self) -> String {
        self.token.clone().unwrap_or(SUI_NATIVE_COIN.to_string())
    }

    // 转移币选择coin
    pub async fn select_coin(&self, provider: &Provider) -> crate::Result<SelectCoinHelper> {
        let coin_type = self.get_coin_type();

        let all_coin = provider
            .get_all_coins_by_owner(&self.from.to_string(), &coin_type)
            .await?;

        let mut select_coins = vec![];
        let mut sum = 0u64;

        for coin in &all_coin {
            sum = sum.checked_add(coin.balance).ok_or_else(|| {
                crate::sui::error::SuiError::TransactionBuild {
                    stage: "select transfer coins",
                    reason: "u64 amount overflow".into(),
                }
            })?;
            select_coins.push(coin.clone());
            if sum >= self.amount {
                break;
            }
        }

        // 验证金额是否满足
        if sum < self.amount {
            return Err(crate::sui::error::SuiError::InsufficientBalance(sum, self.amount).into());
        }

        let gas_coin = if coin_type == SUI_NATIVE_COIN {
            select_coins.clone()
        } else {
            vec![]
        };

        Ok(SelectCoinHelper {
            select_coins,
            gas_coin,
        })
    }

    pub async fn build_pt(
        &self,
        provider: &Provider,
        helper: &mut SelectCoinHelper,
        gas_budget: Option<u64>,
    ) -> crate::Result<ProgrammableTransaction> {
        let mut builder = ProgrammableTransactionBuilder::new();
        let gas_fee = gas_budget.unwrap_or(0);
        let coin_type = self.get_coin_type();

        // 处理gas_coin
        if helper.need_extra_coin_pay_gas(&coin_type, gas_fee, self.amount)? {
            let gas_coin = helper
                .select_gas_coin(gas_fee, provider, &self.from.to_string())
                .await?;
            helper.add_gas_coin(gas_coin);
        };

        // 主币
        let trans_arg = if coin_type == SUI_NATIVE_COIN {
            helper.build_main_coin_arg(&mut builder, self.amount, gas_fee)?
        } else {
            helper.build_token_coin_arg(&mut builder, self.amount)?
        };

        // command
        let receipt = builder.pure(self.to)?;
        builder.command(Command::TransferObjects(TransferObjects {
            objects: trans_arg,
            address: receipt,
        }))?;

        Ok(builder.finish())
    }

    pub async fn build_data(
        &self,
        provider: &Provider,
        mut helper: SelectCoinHelper,
        gas: EstimateFeeResp,
    ) -> crate::Result<Transaction> {
        let gas_budget = gas.get_fee();

        // 携带费用重新计算
        let pt = self
            .build_pt(provider, &mut helper, Some(gas_budget))
            .await?;

        let gas_payment = helper.gas_obj_ref();

        let tx_data = Transaction {
            kind: TransactionKind::ProgrammableTransaction(pt),
            sender: self.from,
            gas_payment: GasPayment {
                objects: gas_payment,
                owner: self.from,
                price: gas.gas_price,
                budget: gas_budget,
            },
            expiration: TransactionExpiration::None,
        };

        Ok(tx_data)
    }
}
