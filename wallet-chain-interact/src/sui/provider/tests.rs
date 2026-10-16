use super::*;
use sui_rpc::proto::sui::rpc::v2::{
    Bcs, CommandOutput, CommandResult, ExecutedTransaction, ExecutionStatus, GetObjectResponse,
    Object, Owner, SimulateTransactionResponse, TransactionEffects, owner::OwnerKind,
};
use sui_sdk_types::{Argument, Command, Input, Mutability};

fn shared_object(version: u64) -> GetObjectResponse {
    let mut owner = Owner::default();
    owner.kind = Some(OwnerKind::Shared as i32);
    owner.version = Some(version);
    let mut object = Object::default();
    object.object_id = Some(
        request::DENY_LIST_OBJECT_ID
            .parse::<Address>()
            .unwrap()
            .to_string(),
    );
    object.owner = Some(owner);
    let mut response = GetObjectResponse::default();
    response.object = Some(object);
    response
}

fn deny_response(current: u8, next: u8) -> SimulateTransactionResponse {
    let mut status = ExecutionStatus::default();
    status.success = Some(true);
    let mut effects = TransactionEffects::default();
    effects.status = Some(status);
    let mut transaction = ExecutedTransaction::default();
    transaction.effects = Some(effects);
    let mut response = SimulateTransactionResponse::default();
    response.transaction = Some(transaction);
    response.command_outputs = [current, next]
        .into_iter()
        .map(|value| {
            let mut bcs = Bcs::default();
            bcs.value = Some(vec![value].into());
            let mut output = CommandOutput::default();
            output.value = Some(bcs);
            let mut result = CommandResult::default();
            result.return_values = vec![output];
            result
        })
        .collect();
    response
}

#[test]
fn deny_query_uses_immutable_shared_object_and_both_epoch_calls() {
    let owner = "0x123".parse::<Address>().unwrap();
    let coin_type = wallet_utils::address::parse_sui_type_tag("0x2::sui::SUI").unwrap();
    let tx = request::deny_list_transaction(coin_type.clone(), owner, 42).unwrap();
    let Input::Shared(shared) = &tx.inputs[0] else {
        panic!("expected shared input")
    };
    assert_eq!(shared.object_id(), "0x403".parse::<Address>().unwrap());
    assert_eq!(shared.version(), 42);
    assert_eq!(shared.mutability(), Mutability::Immutable);
    let Input::Pure(bytes) = &tx.inputs[1] else {
        panic!("expected owner input")
    };
    assert_eq!(bcs::from_bytes::<Address>(bytes).unwrap(), owner);
    assert_eq!(tx.commands.len(), 2);
    for (command, function) in tx.commands.iter().zip([
        "deny_list_v2_contains_current_epoch",
        "deny_list_v2_contains_next_epoch",
    ]) {
        let Command::MoveCall(call) = command else {
            panic!("expected Move call")
        };
        assert_eq!(call.package, "0x2".parse::<Address>().unwrap());
        assert_eq!(call.module.as_str(), "coin");
        assert_eq!(call.function.as_str(), function);
        assert_eq!(call.type_arguments, vec![coin_type.clone()]);
        assert_eq!(call.arguments, vec![Argument::Input(0), Argument::Input(1)]);
    }
    let query = request::deny_list_simulation_request(owner, tx);
    assert_eq!(query.checks, Some(1));
    assert_eq!(query.do_gas_selection, Some(false));
    let transaction = query.transaction.as_ref().unwrap();
    assert_eq!(
        transaction.sender.as_deref(),
        Some(owner.to_string().as_str())
    );
    assert!(transaction.gas_payment.is_none());
    assert!(
        query
            .read_mask
            .unwrap()
            .paths
            .contains(&"command_outputs".to_string())
    );
    assert!(
        request::deny_list_object_request()
            .read_mask
            .unwrap()
            .paths
            .contains(&"owner".to_string())
    );
}

#[test]
fn fee_simulation_keeps_explicit_gas_price() {
    let owner = "0x123".parse::<Address>().unwrap();
    let tx = ProgrammableTransaction {
        inputs: vec![],
        commands: vec![],
    };
    let query = request::kind_simulation_request(&owner.to_string(), tx, 1000).unwrap();
    assert_eq!(
        query.transaction.unwrap().gas_payment.unwrap().price,
        Some(1000)
    );
}

#[test]
fn deny_query_requires_shared_initial_version() {
    assert_eq!(
        mapping::map_deny_list_version(shared_object(42)).unwrap(),
        42
    );
    assert!(mapping::map_deny_list_version(GetObjectResponse::default()).is_err());
    for response in [
        {
            let mut r = shared_object(42);
            r.object.as_mut().unwrap().owner = None;
            r
        },
        {
            let mut r = shared_object(42);
            r.object.as_mut().unwrap().owner.as_mut().unwrap().kind =
                Some(OwnerKind::Address as i32);
            r
        },
        {
            let mut r = shared_object(42);
            r.object.as_mut().unwrap().owner.as_mut().unwrap().version = None;
            r
        },
        shared_object(0),
        {
            let mut r = shared_object(42);
            r.object.as_mut().unwrap().object_id = Some("0x5".into());
            r
        },
    ] {
        assert!(mapping::map_deny_list_version(response).is_err());
    }
}

#[test]
fn deny_status_preserves_current_and_next_epoch_semantics() {
    for (current, next) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let status = mapping::map_deny_status(deny_response(current, next)).unwrap();
        assert_eq!(status.current_epoch, current == 1);
        assert_eq!(status.next_epoch, next == 1);
        assert_eq!(status.is_blacklisted(), current == 1 || next == 1);
        assert_eq!(status.cannot_receive(), current == 1);
        assert_eq!(status.cannot_send(), next == 1);
    }
}

#[test]
fn deny_status_rejects_failed_or_malformed_simulation() {
    for response in [
        SimulateTransactionResponse::default(),
        {
            let mut r = deny_response(0, 0);
            r.transaction
                .as_mut()
                .unwrap()
                .effects
                .as_mut()
                .unwrap()
                .status
                .as_mut()
                .unwrap()
                .success = Some(false);
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.transaction
                .as_mut()
                .unwrap()
                .effects
                .as_mut()
                .unwrap()
                .status
                .as_mut()
                .unwrap()
                .success = None;
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.command_outputs.pop();
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.command_outputs[0].return_values.clear();
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.command_outputs[0].return_values[0].value = None;
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.command_outputs[0].return_values[0]
                .value
                .as_mut()
                .unwrap()
                .value = None;
            r
        },
        {
            let mut r = deny_response(0, 0);
            r.command_outputs[0].return_values[0]
                .value
                .as_mut()
                .unwrap()
                .value = Some(vec![0, 0].into());
            r
        },
        deny_response(2, 0),
    ] {
        assert!(mapping::map_deny_status(response).is_err());
    }
}

#[tokio::test]
async fn deny_query_rejects_invalid_input_before_rpc() {
    let provider = Provider::new("http://127.0.0.1:1", None).unwrap();
    for (coin, owner, field) in [
        ("bad-type", "0x1", "coin_type"),
        ("0x2::sui::SUI", "bad-address", "owner"),
    ] {
        let error = provider.black_address(coin, owner).await.unwrap_err();
        if field == "coin_type" {
            assert!(matches!(error, crate::Error::UtilsError(_)));
        } else {
            assert!(matches!(
                error,
                crate::Error::SuiError(SuiError::InvalidInput { field: "owner", .. })
            ));
        }
    }
}

fn scanner_transaction() -> ExecutedTransaction {
    let mut tx = ExecutedTransaction::default();
    tx.digest = Some(Digest::new([3; 32]).to_string());
    tx.checkpoint = Some(42);
    tx.timestamp.get_or_insert_default().seconds = 1_700_000_000;
    let effects = tx.effects.get_or_insert_default();
    effects.status.get_or_insert_default().success = Some(true);
    let gas = effects.gas_used.get_or_insert_default();
    gas.computation_cost = Some(100);
    gas.storage_cost = Some(20);
    gas.storage_rebate = Some(10);
    let payment = tx
        .transaction
        .get_or_insert_default()
        .gas_payment
        .get_or_insert_default();
    payment.price = Some(1);
    payment.owner = Some("0x3".into());
    let mut change = sui_rpc::proto::sui::rpc::v2::BalanceChange::default();
    change.address = Some("0x1".into());
    change.coin_type = Some("0x2::sui::SUI".into());
    change.amount = Some("-1000".into());
    tx.balance_changes = vec![change];
    tx
}

#[test]
fn scanner_read_masks_include_digests_gas_owner_and_balance_changes() {
    let mask = request::transaction_read_mask();
    assert!(mask.paths.contains(&"transaction.gas_payment.owner".into()));
    assert!(mask.paths.contains(&"balance_changes".into()));
    let mask = request::checkpoint_request("42")
        .unwrap()
        .read_mask
        .unwrap();
    assert!(
        mask.paths
            .contains(&"contents.transactions.transaction".into())
    );
}

#[test]
fn scanner_transaction_preserves_sponsor_and_signed_balances() {
    let tx = scanner_transaction();
    let digest = tx.digest.clone().unwrap();
    let mapped = mapping::map_transaction(tx.clone(), &digest).unwrap();
    assert_eq!(
        mapped.gas_owner,
        "0x3".parse::<Address>().unwrap().to_string()
    );
    assert_eq!(
        mapped.balance_changes[0].address,
        "0x1".parse::<Address>().unwrap().to_string()
    );
    assert_eq!(mapped.balance_changes[0].amount, -1000);
    assert_eq!(
        mapped.balance_changes[0].coin_type,
        "0x2::sui::SUI"
            .parse::<sui_sdk_types::TypeTag>()
            .unwrap()
            .to_string()
    );
    assert_eq!(mapped.net_gas_usage, 110);
    assert_eq!(mapped.timestamp_seconds, 1_700_000_000);
    let mut rebate = tx;
    rebate
        .effects
        .as_mut()
        .unwrap()
        .gas_used
        .as_mut()
        .unwrap()
        .storage_rebate = Some(200);
    assert_eq!(
        mapping::map_transaction(rebate, &digest)
            .unwrap()
            .net_gas_usage,
        -80
    );
}

#[test]
fn scanner_transaction_rejects_missing_or_invalid_balance_fields() {
    let digest = scanner_transaction().digest.unwrap();
    for field in ["owner", "address", "coin_type", "amount"] {
        for malformed in [false, true] {
            let mut tx = scanner_transaction();
            let value = malformed.then(|| "invalid".into());
            match field {
                "owner" => {
                    tx.transaction
                        .as_mut()
                        .unwrap()
                        .gas_payment
                        .as_mut()
                        .unwrap()
                        .owner = value
                }
                "address" => tx.balance_changes[0].address = value,
                "coin_type" => tx.balance_changes[0].coin_type = value,
                "amount" => tx.balance_changes[0].amount = value,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    mapping::map_transaction(tx, &digest),
                    Err(SuiError::InvalidResponse { .. })
                ),
                "{field}, malformed={malformed}"
            );
        }
    }
}

#[test]
fn scanner_checkpoint_maps_ordered_digests_and_rejects_incomplete_contents() {
    use sui_rpc::proto::sui::rpc::v2::{
        Checkpoint, CheckpointedTransactionInfo, GetCheckpointResponse,
    };
    let mut checkpoint = Checkpoint::default();
    checkpoint.sequence_number = Some(42);
    checkpoint.digest = Some(Digest::new([4; 32]).to_string());
    let digests = vec![
        Digest::new([1; 32]).to_string(),
        Digest::new([2; 32]).to_string(),
    ];
    checkpoint.contents.get_or_insert_default().transactions = digests
        .iter()
        .map(|digest| {
            let mut info = CheckpointedTransactionInfo::default();
            info.transaction = Some(digest.clone());
            info
        })
        .collect();
    let mut response = GetCheckpointResponse::default();
    response.checkpoint = Some(checkpoint);
    assert_eq!(
        mapping::map_checkpoint(response.clone(), "42")
            .unwrap()
            .transactions,
        digests
    );
    response
        .checkpoint
        .as_mut()
        .unwrap()
        .contents
        .as_mut()
        .unwrap()
        .transactions[1]
        .transaction = None;
    assert!(mapping::map_checkpoint(response.clone(), "42").is_err());
    response.checkpoint.as_mut().unwrap().contents = None;
    assert!(mapping::map_checkpoint(response.clone(), "42").is_err());
    response.checkpoint.as_mut().unwrap().contents = Some(Default::default());
    assert!(
        mapping::map_checkpoint(response, "42")
            .unwrap()
            .transactions
            .is_empty()
    );
}

#[test]
fn scanner_batch_returns_all_fields_or_fails_as_a_whole() {
    use sui_rpc::proto::sui::rpc::v2::{
        BatchGetTransactionsResponse, GetTransactionResult, get_transaction_result::Result as Item,
    };
    let tx = scanner_transaction();
    let digest = tx.digest.clone().unwrap();
    let mut item = GetTransactionResult::default();
    item.result = Some(Item::Transaction(tx));
    let mut response = BatchGetTransactionsResponse::default();
    response.transactions = vec![item.clone()];
    let mapped = mapping::map_transaction_batch(response.clone(), &[digest.clone()]).unwrap();
    assert_eq!(mapped[0].balance_changes[0].amount, -1000);
    assert!(
        mapping::map_transaction_batch(response.clone(), &[digest.clone(), digest.clone()])
            .is_err()
    );
    assert!(
        mapping::map_transaction_batch(response.clone(), &[Digest::new([8; 32]).to_string()])
            .is_err()
    );
    let Some(Item::Transaction(ref mut tx)) = item.result else {
        unreachable!()
    };
    tx.balance_changes[0].amount = None;
    response.transactions.push(item);
    assert!(mapping::map_transaction_batch(response, &[digest.clone(), digest]).is_err());
}

#[test]
fn scanner_gasless_transfer_preserves_balances_in_single_and_batch_reads() {
    use sui_rpc::proto::sui::rpc::v2::{
        BatchGetTransactionsResponse, GetTransactionResponse, GetTransactionResult,
        get_transaction_result::Result as Item,
    };
    let mut tx = scanner_transaction();
    tx.transaction
        .as_mut()
        .unwrap()
        .gas_payment
        .as_mut()
        .unwrap()
        .price = Some(0);
    tx.transaction
        .as_mut()
        .unwrap()
        .gas_payment
        .as_mut()
        .unwrap()
        .budget = Some(0);
    let gas = tx.effects.as_mut().unwrap().gas_used.as_mut().unwrap();
    gas.computation_cost = Some(1_322_268);
    gas.storage_cost = Some(0);
    gas.storage_rebate = Some(1_322_268);
    let coin = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
    tx.balance_changes[0].coin_type = Some(coin.into());
    tx.balance_changes[0].amount = Some("-241526517".into());
    let mut recipient = tx.balance_changes[0].clone();
    recipient.address = Some("0x2".into());
    recipient.amount = Some("241526517".into());
    tx.balance_changes.push(recipient);
    let digest = tx.digest.clone().unwrap();
    let mut single = GetTransactionResponse::default();
    single.transaction = Some(tx.clone());
    let mapped = mapping::map_transaction_response(single, &digest).unwrap();
    assert_eq!(mapped.gas_price, 0);
    assert_eq!(mapped.net_gas_usage, 0);
    assert_eq!(mapped.balance_changes.len(), 2);
    assert_eq!(mapped.balance_changes[0].coin_type, coin);
    assert_eq!(mapped.balance_changes[0].amount, -241526517);
    assert_eq!(mapped.balance_changes[1].amount, 241526517);

    let normal = scanner_transaction();
    let mut normal = normal;
    // A different digest ensures batch ordering/correspondence is checked too.
    normal.digest = Some(Digest::new([9; 32]).to_string());
    let mut batch = BatchGetTransactionsResponse::default();
    batch.transactions = [normal.clone(), tx]
        .into_iter()
        .map(|tx| {
            let mut item = GetTransactionResult::default();
            item.result = Some(Item::Transaction(tx));
            item
        })
        .collect();
    let results = mapping::map_transaction_batch(batch, &[normal.digest.unwrap(), digest]).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].gas_price, 1);
    assert_eq!(results[1], mapped);
}

#[test]
fn scanner_transaction_still_rejects_missing_or_overflowing_gas_price() {
    for price in [None, Some(i64::MAX as u64 + 1)] {
        let mut tx = scanner_transaction();
        let digest = tx.digest.clone().unwrap();
        tx.transaction
            .as_mut()
            .unwrap()
            .gas_payment
            .as_mut()
            .unwrap()
            .price = price;
        assert!(matches!(
            mapping::map_transaction(tx, &digest),
            Err(SuiError::InvalidResponse {
                field: "transaction.gas_payment.price",
                ..
            })
        ));
    }
}
