use super::{Provider, consts::SUI_NATIVE_COIN, protocol::SuiCoin};
use sui_sdk_types::{
    Argument, Command, Input, MergeCoins, ObjectReference, ProgrammableTransaction, SplitCoins,
};

fn amount_overflow(stage: &'static str) -> crate::Error {
    crate::sui::error::SuiError::TransactionBuild {
        stage,
        reason: "u64 amount overflow".into(),
    }
    .into()
}

fn checked_coin_sum<'a>(
    coins: impl IntoIterator<Item = &'a SuiCoin>,
    stage: &'static str,
) -> crate::Result<u64> {
    coins.into_iter().try_fold(0u64, |sum, coin| {
        sum.checked_add(coin.balance)
            .ok_or_else(|| amount_overflow(stage))
    })
}

pub struct ProgrammableTransactionBuilder {
    inputs: Vec<Input>,
    commands: Vec<Command>,
}

impl ProgrammableTransactionBuilder {
    pub fn new() -> Self {
        Self {
            inputs: Vec::new(),
            commands: Vec::new(),
        }
    }

    pub fn pure<T: serde::Serialize>(&mut self, value: T) -> crate::Result<Argument> {
        let bytes = wallet_utils::serde_func::bcs_to_bytes(&value).map_err(|error| {
            crate::sui::error::SuiError::TransactionBuild {
                stage: "serialize pure input",
                reason: error.to_string(),
            }
        })?;
        self.input(Input::Pure(bytes))
    }

    pub fn object(&mut self, object: ObjectReference) -> crate::Result<Argument> {
        self.input(Input::ImmutableOrOwned(object))
    }

    fn input(&mut self, input: Input) -> crate::Result<Argument> {
        let index = u16::try_from(self.inputs.len())
            .map_err(|_| crate::sui::error::SuiError::TooManyProgrammableInputs)?;
        self.inputs.push(input);
        Ok(Argument::Input(index))
    }

    pub fn command(&mut self, command: Command) -> crate::Result<Argument> {
        let index = u16::try_from(self.commands.len())
            .map_err(|_| crate::sui::error::SuiError::TooManyProgrammableCommands)?;
        self.commands.push(command);
        Ok(Argument::Result(index))
    }

    pub fn finish(self) -> ProgrammableTransaction {
        ProgrammableTransaction {
            inputs: self.inputs,
            commands: self.commands,
        }
    }
}

pub struct SelectCoinHelper {
    // 选择的coin
    pub select_coins: Vec<SuiCoin>,
    // gas_coin
    pub gas_coin: Vec<SuiCoin>,
}

impl SelectCoinHelper {
    // 选择代币的总金额
    pub fn select_total_amount(&self) -> crate::Result<u64> {
        checked_coin_sum(&self.select_coins, "sum selected coins")
    }

    pub fn gas_amount(&self) -> crate::Result<u64> {
        checked_coin_sum(&self.gas_coin, "sum gas coins")
    }

    // 选择代币和gas的总金额
    pub fn select_total_with_gas(&self) -> crate::Result<u64> {
        self.select_total_amount()?
            .checked_add(self.gas_amount()?)
            .ok_or_else(|| amount_overflow("sum selected and gas coins"))
    }

    // true 需要单独一个对象支付gas false 不需要
    pub fn need_extra_coin_pay_gas(
        &self,
        coin_type: &str,
        gas_fee: u64,
        amount: u64,
    ) -> crate::Result<bool> {
        let check_amount = if coin_type == SUI_NATIVE_COIN {
            gas_fee
                .checked_add(amount)
                .ok_or_else(|| amount_overflow("sum transfer amount and gas fee"))?
        } else {
            gas_fee
        };

        let gas_amount = self.gas_amount()?;

        Ok(gas_amount < check_amount)
    }

    // 增加gas coin into select coin
    // notes select coin must be main coin
    pub fn add_gas_coin(&mut self, gas_coin: Vec<SuiCoin>) {
        self.gas_coin.extend(gas_coin);
    }

    // 提出到已经选择的coin,在选择其他sui币作为手续费
    pub async fn select_gas_coin(
        &self,
        gas_fee: u64,
        provider: &Provider,
        owner: &str,
    ) -> crate::Result<Vec<SuiCoin>> {
        let coin = provider
            .get_all_coins_by_owner(owner, SUI_NATIVE_COIN)
            .await?;

        let unselect_coin = coin
            .into_iter()
            .filter(|c| !self.select_coins.contains(c))
            .collect::<Vec<SuiCoin>>();

        // 加上原来的gas
        let mut sum = self.gas_amount()?;
        let mut gas_coin = vec![];
        for coin in unselect_coin {
            sum = sum
                .checked_add(coin.balance)
                .ok_or_else(|| amount_overflow("select gas coins"))?;
            gas_coin.push(coin);
            if sum >= gas_fee {
                break;
            }
        }
        if sum < gas_fee {
            return Err(crate::sui::error::SuiError::InsufficientFee(sum, gas_fee).into());
        }

        Ok(gas_coin)
    }

    pub fn gas_obj_ref(&self) -> Vec<ObjectReference> {
        self.gas_coin.iter().map(|c| c.object_ref()).collect()
    }

    pub fn build_main_coin_arg(
        &mut self,
        builder: &mut ProgrammableTransactionBuilder,
        amount: u64,
        gas_fee: u64,
    ) -> crate::Result<Vec<Argument>> {
        // 是否进行拆分币(绝大多情况下是需要的)
        let required = gas_fee
            .checked_add(amount)
            .ok_or_else(|| amount_overflow("sum transfer amount and gas fee"))?;
        if self.select_total_with_gas()? > required {
            let pure_amount = builder.pure(amount)?;
            Ok(vec![builder.command(Command::SplitCoins(SplitCoins {
                coin: Argument::Gas,
                amounts: vec![pure_amount],
            }))?])
        } else {
            Ok(vec![Argument::Gas])
        }
    }

    // 代币转账构建转账的arg
    pub fn build_token_coin_arg(
        &mut self,
        builder: &mut ProgrammableTransactionBuilder,
        amount: u64,
    ) -> crate::Result<Vec<Argument>> {
        // 代币
        let mut coin_inputs = vec![];
        for coin in self.select_coins.iter() {
            let coin_arg = builder.object(coin.object_ref())?;
            coin_inputs.push(coin_arg);
        }

        // merge_coin
        let primary = coin_inputs.first().copied().ok_or_else(|| {
            crate::sui::error::SuiError::InvalidInput {
                field: "selected_coins",
                reason: "token transfer requires at least one coin".into(),
            }
        })?;
        if coin_inputs.len() > 1 {
            builder.command(Command::MergeCoins(MergeCoins {
                coin: primary,
                coins_to_merge: coin_inputs[1..].to_vec(),
            }))?;
        }

        //
        let transfer_amount = if self.select_total_amount()? > amount {
            let pure_amount = builder.pure(amount)?;

            vec![builder.command(Command::SplitCoins(SplitCoins {
                coin: primary,
                amounts: vec![pure_amount],
            }))?]
        } else {
            vec![primary]
        };

        Ok(transfer_amount)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sui_sdk_types::{Digest, ObjectReference};

    fn coin(id: &str, balance: u64) -> SuiCoin {
        SuiCoin {
            object_ref: ObjectReference::new(id.parse().unwrap(), 1, Digest::ZERO),
            balance,
        }
    }

    #[test]
    fn token_builder_rejects_empty_coin_selection_without_panicking() {
        let mut helper = SelectCoinHelper {
            select_coins: Vec::new(),
            gas_coin: Vec::new(),
        };
        let error = helper
            .build_token_coin_arg(&mut ProgrammableTransactionBuilder::new(), 1)
            .unwrap_err();
        assert!(matches!(
            error,
            crate::Error::SuiError(crate::sui::error::SuiError::InvalidInput {
                field: "selected_coins",
                ..
            })
        ));
    }

    #[test]
    fn coin_totals_reject_u64_overflow() {
        let helper = SelectCoinHelper {
            select_coins: vec![coin("0x1", u64::MAX), coin("0x2", 1)],
            gas_coin: Vec::new(),
        };
        assert!(matches!(
            helper.select_total_amount().unwrap_err(),
            crate::Error::SuiError(crate::sui::error::SuiError::TransactionBuild {
                stage: "sum selected coins",
                ..
            })
        ));
    }
}
