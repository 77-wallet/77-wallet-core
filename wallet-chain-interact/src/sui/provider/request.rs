use sui_rpc::field::{FieldMask, FieldMaskUtil};
use sui_rpc::proto::sui::rpc::v2::{
    Bcs, ExecuteTransactionRequest, GasPayment as GrpcGasPayment, GetCheckpointRequest,
    ListOwnedObjectsRequest, SimulateTransactionRequest, Transaction as GrpcTransaction,
    TransactionKind as GrpcTransactionKind, UserSignature as GrpcUserSignature,
    get_checkpoint_request, simulate_transaction_request,
};
use sui_sdk_types::bcs::FromBcs;
use sui_sdk_types::{Address, ProgrammableTransaction, Transaction, TransactionKind};
use sui_sdk_types::{Argument, Command, Input, MoveCall, SharedInput, TypeTag};

pub(super) const DENY_LIST_OBJECT_ID: &str = "0x403";

pub(super) fn deny_list_object_request() -> sui_rpc::proto::sui::rpc::v2::GetObjectRequest {
    let mut request = sui_rpc::proto::sui::rpc::v2::GetObjectRequest::default();
    request.object_id = Some(DENY_LIST_OBJECT_ID.parse::<Address>().unwrap().to_string());
    request.read_mask = Some(FieldMask::from_paths(["object_id", "owner"]));
    request
}

pub(super) fn deny_list_transaction(
    coin_type: TypeTag,
    owner: Address,
    initial_shared_version: u64,
) -> crate::Result<ProgrammableTransaction> {
    let owner_bytes = bcs::to_bytes(&owner).map_err(|error| SuiError::TransactionBuild {
        stage: "serialize deny-list owner",
        reason: error.to_string(),
    })?;
    Ok(ProgrammableTransaction {
        inputs: vec![
            Input::Shared(SharedInput::new(
                DENY_LIST_OBJECT_ID.parse::<Address>().unwrap(),
                initial_shared_version,
                false,
            )),
            Input::Pure(owner_bytes),
        ],
        commands: [
            "deny_list_v2_contains_current_epoch",
            "deny_list_v2_contains_next_epoch",
        ]
        .into_iter()
        .map(|function| {
            Command::MoveCall(MoveCall {
                package: "0x2".parse().unwrap(),
                module: "coin".parse().unwrap(),
                function: function.parse().unwrap(),
                type_arguments: vec![coin_type.clone()],
                arguments: vec![Argument::Input(0), Argument::Input(1)],
            })
        })
        .collect(),
    })
}

pub(super) fn deny_list_simulation_request(
    sender: Address,
    tx: ProgrammableTransaction,
) -> SimulateTransactionRequest {
    let mut transaction = GrpcTransaction::default();
    transaction.kind = Some(GrpcTransactionKind::from(
        TransactionKind::ProgrammableTransaction(tx),
    ));
    transaction.sender = Some(sender.to_string());
    // The node resolves gas defaults for structured simulation requests.
    // This read-only query needs neither a gas-price RPC nor real gas selection.
    let mut request = simulation_request(
        transaction,
        simulate_transaction_request::TransactionChecks::Disabled,
    );
    request.read_mask = Some(FieldMask::from_paths([
        "transaction.effects.status",
        "command_outputs",
    ]));
    request
}

pub(super) fn coin_list_request(
    owner: &str,
    coin_type: &str,
    cursor: Option<Vec<u8>>,
) -> ListOwnedObjectsRequest {
    let mut request = ListOwnedObjectsRequest::default();
    request.owner = Some(owner.to_string());
    request.object_type = Some(format!("0x2::coin::Coin<{coin_type}>"));
    request.page_size = Some(50);
    request.page_token = cursor.map(Into::into);
    request.read_mask = Some(FieldMask::from_paths([
        "object_id",
        "version",
        "digest",
        "balance",
    ]));
    request
}

pub(super) fn parse_object_type_filter(
    filter: Option<serde_json::Value>,
) -> crate::Result<Option<String>> {
    let Some(filter) = filter else {
        return Ok(None);
    };
    let Some(map) = filter.as_object() else {
        return Err(SuiError::Unsupported {
            feature: "owned-object filter",
        }
        .into());
    };
    let Some(object_type) = map.get("StructType").and_then(|value| value.as_str()) else {
        return Err(SuiError::Unsupported {
            feature: "owned-object filter",
        }
        .into());
    };
    if map.len() != 1 || object_type.is_empty() {
        return Err(SuiError::Unsupported {
            feature: "owned-object filter",
        }
        .into());
    }
    Ok(Some(object_type.to_string()))
}

pub(super) fn checkpoint_request(check_point: &str) -> crate::Result<GetCheckpointRequest> {
    let mut request = GetCheckpointRequest::default();
    request.checkpoint_id = Some(if let Ok(sequence_number) = check_point.parse::<u64>() {
        get_checkpoint_request::CheckpointId::SequenceNumber(sequence_number)
    } else {
        if check_point.is_empty() {
            return Err(SuiError::InvalidInput {
                field: "checkpoint_id",
                reason: "must not be empty".into(),
            }
            .into());
        }
        get_checkpoint_request::CheckpointId::Digest(check_point.to_string())
    });
    request.read_mask = Some(FieldMask::from_paths([
        "sequence_number",
        "digest",
        "contents.transactions.transaction",
    ]));
    Ok(request)
}

pub(super) fn transaction_read_mask() -> FieldMask {
    FieldMask::from_paths([
        "digest",
        "checkpoint",
        "timestamp",
        "effects.status",
        "effects.gas_used",
        "transaction.gas_payment.price",
        "transaction.gas_payment.owner",
        "balance_changes",
    ])
}

fn simulation_request(
    transaction: GrpcTransaction,
    checks: simulate_transaction_request::TransactionChecks,
) -> SimulateTransactionRequest {
    let mut request = SimulateTransactionRequest::new(transaction);
    request.read_mask = Some(FieldMask::from_paths([
        "transaction.effects.status",
        "transaction.effects.gas_used",
    ]));
    request.checks = Some(checks as i32);
    request.do_gas_selection = Some(false);
    request
}

pub(super) fn full_simulation_request(
    tx_data: &Transaction,
) -> crate::Result<SimulateTransactionRequest> {
    let bytes = wallet_utils::serde_func::bcs_to_bytes(tx_data).map_err(|error| {
        SuiError::TransactionBuild {
            stage: "serialize transaction",
            reason: error.to_string(),
        }
    })?;
    let mut transaction = GrpcTransaction::default();
    let mut bcs = Bcs::default();
    bcs.name = Some("TransactionData".into());
    bcs.value = Some(bytes.into());
    transaction.bcs = Some(bcs);
    Ok(simulation_request(
        transaction,
        simulate_transaction_request::TransactionChecks::Enabled,
    ))
}

pub(super) fn kind_simulation_request(
    sender: &str,
    tx: ProgrammableTransaction,
    gas_price: u64,
) -> crate::Result<SimulateTransactionRequest> {
    let sender = sender
        .parse::<Address>()
        .map_err(|error| SuiError::InvalidInput {
            field: "sender",
            reason: error.to_string(),
        })?
        .to_string();
    let kind = TransactionKind::ProgrammableTransaction(tx);
    let mut transaction = GrpcTransaction::default();
    transaction.kind = Some(GrpcTransactionKind::from(kind));
    transaction.sender = Some(sender.clone());
    let mut payment = GrpcGasPayment::default();
    payment.owner = Some(sender);
    payment.price = Some(gas_price);
    transaction.gas_payment = Some(payment);
    Ok(simulation_request(
        transaction,
        simulate_transaction_request::TransactionChecks::Disabled,
    ))
}

pub(super) fn execution_request(
    tx_bytes: Vec<u8>,
    signatures: Vec<Vec<u8>>,
) -> crate::Result<(ExecuteTransactionRequest, String)> {
    if tx_bytes.is_empty() || signatures.is_empty() || signatures.iter().any(Vec::is_empty) {
        return Err(SuiError::InvalidInput {
            field: "transaction/signatures",
            reason: "transaction and every signature must be non-empty".into(),
        }
        .into());
    }
    let digest = Transaction::from_bcs(&tx_bytes)
        .map_err(|error| SuiError::TransactionBuild {
            stage: "decode transaction BCS",
            reason: error.to_string(),
        })?
        .digest()
        .to_string();
    let mut transaction = GrpcTransaction::default();
    let mut tx_bcs = Bcs::default();
    tx_bcs.name = Some("TransactionData".into());
    tx_bcs.value = Some(tx_bytes.into());
    transaction.bcs = Some(tx_bcs);
    let mut request = ExecuteTransactionRequest::new(transaction);
    request.signatures = signatures
        .into_iter()
        .map(|bytes| {
            let mut bcs = Bcs::default();
            bcs.name = Some("UserSignatureBytes".into());
            bcs.value = Some(bytes.into());
            let mut signature = GrpcUserSignature::default();
            signature.bcs = Some(bcs);
            signature
        })
        .collect();
    request.read_mask = Some(FieldMask::from_paths(["digest", "effects.status"]));
    Ok((request, digest))
}
use crate::sui::error::SuiError;
