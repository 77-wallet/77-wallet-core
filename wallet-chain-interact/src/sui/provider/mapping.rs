use crate::sui::error::{ExecutionPhase, SuiError};
use crate::sui::protocol::{
    CheckpointResult, SuiBalance, SuiBalanceChange, SuiCoin, SuiCoinMetadata, SuiDenyStatus,
    SuiObject, SuiOwnedObject, SuiPackage, SuiPackageModule, SuiQueriedTransaction, SuiSimulation,
};
use std::collections::BTreeMap;
use sui_rpc::proto::sui::rpc::v2::{
    BatchGetTransactionsResponse, ExecuteTransactionResponse, ExecutedTransaction,
    GetBalanceResponse, GetCheckpointResponse, GetCoinInfoResponse, GetEpochResponse,
    GetObjectResponse, GetPackageResponse, GetServiceInfoResponse, Object as GrpcObject,
    SimulateTransactionResponse, get_transaction_result,
};
use sui_sdk_types::{Address, Digest, ObjectReference};
use tonic::Code;

pub(super) fn map_deny_list_version(response: GetObjectResponse) -> Result<u64, SuiError> {
    let operation = "get deny-list object";
    let object = response.object.ok_or_else(|| SuiError::NotFound {
        resource: "deny-list object",
        id: super::request::DENY_LIST_OBJECT_ID.into(),
    })?;
    let id = required(object.object_id, operation, "object_id")?
        .parse::<Address>()
        .map_err(|_| invalid(operation, "object_id", "invalid address"))?;
    if id
        != super::request::DENY_LIST_OBJECT_ID
            .parse::<Address>()
            .unwrap()
    {
        return Err(invalid(
            operation,
            "object_id",
            "unexpected deny-list object",
        ));
    }
    let owner = required(object.owner, operation, "owner")?;
    if owner.kind != Some(sui_rpc::proto::sui::rpc::v2::owner::OwnerKind::Shared as i32) {
        return Err(invalid(
            operation,
            "owner.kind",
            "deny-list object must be shared",
        ));
    }
    let version = required(owner.version, operation, "owner.version")?;
    if version == 0 {
        return Err(invalid(operation, "owner.version", "must be non-zero"));
    }
    Ok(version)
}

pub(super) fn map_deny_status(
    response: SimulateTransactionResponse,
) -> Result<SuiDenyStatus, SuiError> {
    let operation = "simulate deny-list query";
    let transaction = required(response.transaction, operation, "transaction")?;
    let effects = required(transaction.effects, operation, "transaction.effects")?;
    let status = required(effects.status, operation, "transaction.effects.status")?;
    if !required(
        status.success,
        operation,
        "transaction.effects.status.success",
    )? {
        return Err(SuiError::ExecutionFailed {
            phase: ExecutionPhase::Simulation,
            reason: status
                .error
                .and_then(|error| error.description)
                .unwrap_or_else(|| "deny-list query execution failed".into()),
        });
    }
    if response.command_outputs.len() != 2 {
        return Err(invalid(
            operation,
            "command_outputs",
            "expected two epoch results",
        ));
    }
    let parse_bool = |index: usize| -> Result<bool, SuiError> {
        let values = &response.command_outputs[index].return_values;
        if values.len() != 1 {
            return Err(invalid(
                operation,
                "return_values",
                "expected one bool per epoch",
            ));
        }
        let value = required(values[0].value.as_ref(), operation, "return_values.value")?;
        let bytes = required(value.value.as_ref(), operation, "return_values.value.value")?;
        bcs::from_bytes::<bool>(bytes)
            .map_err(|error| invalid(operation, "return_values", error.to_string()))
    };
    Ok(SuiDenyStatus {
        current_epoch: parse_bool(0)?,
        next_epoch: parse_bool(1)?,
    })
}

fn required<T>(
    value: Option<T>,
    operation: &'static str,
    field: &'static str,
) -> Result<T, SuiError> {
    value.ok_or_else(|| SuiError::invalid_response(operation, field, "missing"))
}

fn invalid(operation: &'static str, field: &'static str, reason: impl Into<String>) -> SuiError {
    SuiError::invalid_response(operation, field, reason)
}

impl TryFrom<GetBalanceResponse> for SuiBalance {
    type Error = SuiError;

    fn try_from(response: GetBalanceResponse) -> Result<Self, Self::Error> {
        let balance = required(response.balance, "get balance", "balance")?;
        let total_balance = required(balance.balance, "get balance", "balance.balance")?;
        Ok(Self { total_balance })
    }
}

impl TryFrom<GetCoinInfoResponse> for SuiCoinMetadata {
    type Error = SuiError;

    fn try_from(response: GetCoinInfoResponse) -> Result<Self, Self::Error> {
        let metadata = required(response.metadata, "get coin info", "metadata")?;
        let decimals = required(metadata.decimals, "get coin info", "metadata.decimals")?
            .try_into()
            .map_err(|_| invalid("get coin info", "metadata.decimals", "exceeds u8"))?;
        let symbol = required(metadata.symbol, "get coin info", "metadata.symbol")?;
        let name = required(metadata.name, "get coin info", "metadata.name")?;
        Ok(Self {
            decimals,
            symbol,
            name,
        })
    }
}

pub(super) fn object_ref(object: &GrpcObject) -> Result<ObjectReference, SuiError> {
    let id = required(object.object_id.as_deref(), "map object", "object_id")?
        .parse::<Address>()
        .map_err(|_| invalid("map object", "object_id", "invalid address"))?;
    let version = required(object.version, "map object", "version")?;
    let digest = required(object.digest.as_deref(), "map object", "digest")?
        .parse::<Digest>()
        .map_err(|_| invalid("map object", "digest", "invalid digest"))?;
    Ok(ObjectReference::new(id, version, digest))
}

impl TryFrom<GrpcObject> for SuiCoin {
    type Error = SuiError;

    fn try_from(object: GrpcObject) -> Result<Self, Self::Error> {
        let object_ref = object_ref(&object)?;
        let balance = required(object.balance, "map coin", "balance")?;
        Ok(Self {
            object_ref,
            balance,
        })
    }
}

impl TryFrom<GrpcObject> for SuiOwnedObject {
    type Error = SuiError;

    fn try_from(object: GrpcObject) -> Result<Self, Self::Error> {
        let object_ref = object_ref(&object)?;
        let object_type = required(object.object_type, "map owned object", "object_type")?;
        Ok(Self {
            object_ref,
            object_type,
        })
    }
}

pub(super) fn map_checkpoint_height(response: GetServiceInfoResponse) -> Result<u64, SuiError> {
    required(
        response.checkpoint_height,
        "get service info",
        "checkpoint_height",
    )
}

pub(super) fn map_checkpoint(
    response: GetCheckpointResponse,
    requested_id: &str,
) -> Result<CheckpointResult, SuiError> {
    let checkpoint = response.checkpoint.ok_or_else(|| SuiError::NotFound {
        resource: "checkpoint",
        id: requested_id.to_string(),
    })?;
    let contents = required(checkpoint.contents, "get checkpoint", "checkpoint.contents")?;
    let transactions = contents
        .transactions
        .into_iter()
        .map(|info| {
            let digest = required(
                info.transaction,
                "get checkpoint",
                "contents.transactions.transaction",
            )?;
            digest
                .parse::<Digest>()
                .map(|digest| digest.to_string())
                .map_err(|error| {
                    invalid(
                        "get checkpoint",
                        "contents.transactions.transaction",
                        error.to_string(),
                    )
                })
        })
        .collect::<Result<Vec<_>, SuiError>>()?;

    Ok(CheckpointResult {
        sequence_number: required(
            checkpoint.sequence_number,
            "get checkpoint",
            "checkpoint.sequence_number",
        )?,
        digest: required(checkpoint.digest, "get checkpoint", "checkpoint.digest")?,
        transactions,
    })
}

pub(super) fn map_object_response(
    response: GetObjectResponse,
    requested_id: &str,
) -> Result<SuiObject, SuiError> {
    let object = response.object.ok_or_else(|| SuiError::NotFound {
        resource: "object",
        id: requested_id.to_string(),
    })?;
    let object_ref = object_ref(&object)?;
    Ok(SuiObject { object_ref })
}

pub(super) fn map_transaction(
    transaction: ExecutedTransaction,
    requested_digest: &str,
) -> Result<SuiQueriedTransaction, SuiError> {
    let digest = required(transaction.digest, "get transaction", "transaction.digest")?;
    if digest != requested_digest {
        return Err(invalid(
            "get transaction",
            "transaction.digest",
            format!("expected {requested_digest}, got {digest}"),
        ));
    }
    let checkpoint = required(
        transaction.checkpoint,
        "get transaction",
        "transaction.checkpoint",
    )?;
    let timestamp = required(
        transaction.timestamp,
        "get transaction",
        "transaction.timestamp",
    )?;
    if timestamp.seconds < 0 || !(0..1_000_000_000).contains(&timestamp.nanos) {
        return Err(invalid(
            "get transaction",
            "transaction.timestamp",
            "out of range",
        ));
    }
    let effects = required(
        transaction.effects,
        "get transaction",
        "transaction.effects",
    )?;
    let status = required(
        effects.status,
        "get transaction",
        "transaction.effects.status",
    )?;
    let success = required(
        status.success,
        "get transaction",
        "transaction.effects.status.success",
    )?;
    let gas = required(
        effects.gas_used,
        "get transaction",
        "transaction.effects.gas_used",
    )?;
    let computation = required(
        gas.computation_cost,
        "get transaction",
        "transaction.effects.gas_used.computation_cost",
    )?;
    let storage = required(
        gas.storage_cost,
        "get transaction",
        "transaction.effects.gas_used.storage_cost",
    )?;
    let rebate = required(
        gas.storage_rebate,
        "get transaction",
        "transaction.effects.gas_used.storage_rebate",
    )?;
    let net = i128::from(computation) + i128::from(storage) - i128::from(rebate);
    let net_gas_usage = i64::try_from(net)
        .map_err(|_| invalid("get transaction", "net_gas_usage", "exceeds i64"))?;
    let payment = transaction
        .transaction
        .and_then(|tx| tx.gas_payment)
        .ok_or_else(|| invalid("get transaction", "transaction.gas_payment", "missing"))?;
    let gas_price = required(
        payment.price,
        "get transaction",
        "transaction.gas_payment.price",
    )?;
    let gas_owner = required(
        payment.owner,
        "get transaction",
        "transaction.gas_payment.owner",
    )?
    .parse::<Address>()
    .map_err(|error| {
        invalid(
            "get transaction",
            "transaction.gas_payment.owner",
            error.to_string(),
        )
    })?
    .to_string();
    let balance_changes = transaction
        .balance_changes
        .into_iter()
        .map(|change| {
            let address = required(change.address, "get transaction", "balance_changes.address")?
                .parse::<Address>()
                .map_err(|error| {
                    invalid(
                        "get transaction",
                        "balance_changes.address",
                        error.to_string(),
                    )
                })?
                .to_string();
            let coin_type = required(
                change.coin_type,
                "get transaction",
                "balance_changes.coin_type",
            )?
            .parse::<sui_sdk_types::TypeTag>()
            .map_err(|error| {
                invalid(
                    "get transaction",
                    "balance_changes.coin_type",
                    error.to_string(),
                )
            })?
            .to_string();
            let amount = required(change.amount, "get transaction", "balance_changes.amount")?
                .parse::<i128>()
                .map_err(|error| {
                    invalid(
                        "get transaction",
                        "balance_changes.amount",
                        error.to_string(),
                    )
                })?;
            Ok(SuiBalanceChange {
                address,
                coin_type,
                amount,
            })
        })
        .collect::<Result<Vec<_>, SuiError>>()?;
    // Executed gasless transfers can legitimately have a zero gas price.
    if gas_price > i64::MAX as u64 {
        return Err(invalid(
            "get transaction",
            "transaction.gas_payment.price",
            "must be between 0 and i64::MAX",
        ));
    }
    Ok(SuiQueriedTransaction {
        digest,
        checkpoint,
        timestamp_seconds: timestamp.seconds as u64,
        success,
        net_gas_usage,
        gas_price,
        gas_owner,
        balance_changes,
    })
}

pub(super) fn map_transaction_response(
    response: sui_rpc::proto::sui::rpc::v2::GetTransactionResponse,
    requested_digest: &str,
) -> Result<SuiQueriedTransaction, SuiError> {
    let transaction = response.transaction.ok_or_else(|| SuiError::NotFound {
        resource: "transaction",
        id: requested_digest.to_string(),
    })?;
    map_transaction(transaction, requested_digest)
}

pub(super) fn map_transaction_batch(
    response: BatchGetTransactionsResponse,
    digests: &[String],
) -> Result<Vec<SuiQueriedTransaction>, SuiError> {
    if response.transactions.len() != digests.len() {
        return Err(invalid(
            "batch get transactions",
            "transactions",
            format!(
                "returned {} results for {} digests",
                response.transactions.len(),
                digests.len()
            ),
        ));
    }
    response
        .transactions
        .into_iter()
        .zip(digests)
        .enumerate()
        .map(|(index, (result, digest))| {
            let transaction = match result.result {
                Some(get_transaction_result::Result::Transaction(transaction)) => transaction,
                Some(get_transaction_result::Result::Error(error)) => {
                    if error.code == Code::NotFound as i32 {
                        return Err(SuiError::NotFound {
                            resource: "transaction",
                            id: digest.clone(),
                        });
                    }
                    return Err(invalid(
                        "batch get transactions",
                        "transactions.result",
                        format!(
                            "item {index} ({digest}) failed: code {}: {}",
                            error.code, error.message
                        ),
                    ));
                }
                None => {
                    return Err(invalid(
                        "batch get transactions",
                        "transactions.result",
                        format!("item {index} ({digest}) is missing"),
                    ));
                }
                Some(_) => {
                    return Err(invalid(
                        "batch get transactions",
                        "transactions.result",
                        format!("item {index} ({digest}) is unsupported"),
                    ));
                }
            };
            map_transaction(transaction, digest).map_err(|error| {
                invalid(
                    "batch get transactions",
                    "transactions.item",
                    format!("item {index} ({digest}): {error}"),
                )
            })
        })
        .collect()
}

pub(super) fn map_reference_gas_price(response: GetEpochResponse) -> Result<u64, SuiError> {
    let epoch = required(response.epoch, "get epoch", "epoch")?;
    let price = required(
        epoch.reference_gas_price,
        "get epoch",
        "epoch.reference_gas_price",
    )?;
    if price == 0 {
        return Err(invalid(
            "get epoch",
            "epoch.reference_gas_price",
            "must not be zero",
        ));
    }
    Ok(price)
}

pub(super) fn map_simulation(
    response: SimulateTransactionResponse,
) -> Result<SuiSimulation, SuiError> {
    let transaction = required(response.transaction, "simulate transaction", "transaction")?;
    let effects = required(
        transaction.effects,
        "simulate transaction",
        "transaction.effects",
    )?;
    let status = required(
        effects.status,
        "simulate transaction",
        "transaction.effects.status",
    )?;
    let success = required(
        status.success,
        "simulate transaction",
        "transaction.effects.status.success",
    )?;
    let error = if success {
        None
    } else {
        Some(
            status
                .error
                .and_then(|error| error.description)
                .unwrap_or_else(|| "transaction execution failed".into()),
        )
    };
    let gas = required(
        effects.gas_used,
        "simulate transaction",
        "transaction.effects.gas_used",
    )?;
    let computation_cost = required(
        gas.computation_cost,
        "simulate transaction",
        "transaction.effects.gas_used.computation_cost",
    )?;
    let storage_cost = required(
        gas.storage_cost,
        "simulate transaction",
        "transaction.effects.gas_used.storage_cost",
    )?;
    Ok(SuiSimulation {
        success,
        error,
        computation_cost,
        storage_cost,
    })
}

pub(super) fn map_package(
    response: GetPackageResponse,
    requested_id: Address,
) -> Result<SuiPackage, SuiError> {
    let package = response.package.ok_or_else(|| SuiError::NotFound {
        resource: "package",
        id: requested_id.to_string(),
    })?;
    let storage_id = required(package.storage_id, "get package", "package.storage_id")?
        .parse::<Address>()
        .map_err(|_| invalid("get package", "package.storage_id", "invalid address"))?;
    if storage_id != requested_id {
        return Err(invalid(
            "get package",
            "package.storage_id",
            format!("expected {requested_id}, got {storage_id}"),
        ));
    }
    let original_id = required(package.original_id, "get package", "package.original_id")?
        .parse::<Address>()
        .map_err(|_| invalid("get package", "package.original_id", "invalid address"))?;
    let version = required(package.version, "get package", "package.version")?;
    let mut modules = BTreeMap::new();
    for module in package.modules {
        let name = module
            .name
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("get package", "package.modules.name", "missing or empty"))?;
        let bytecode = module.contents.map(|contents| contents.to_vec());
        if bytecode
            .as_ref()
            .is_some_and(|contents| contents.is_empty())
        {
            return Err(invalid(
                "get package",
                "package.modules.contents",
                format!("module {name} has empty bytecode"),
            ));
        }
        if modules
            .insert(name.clone(), SuiPackageModule { bytecode })
            .is_some()
        {
            return Err(invalid(
                "get package",
                "package.modules.name",
                format!("duplicate module {name}"),
            ));
        }
    }
    Ok(SuiPackage {
        storage_id: storage_id.to_string(),
        original_id: original_id.to_string(),
        version,
        modules,
    })
}

pub(super) fn map_execution_result(
    response: ExecuteTransactionResponse,
    submitted_digest: &str,
) -> Result<String, SuiError> {
    let transaction = required(response.transaction, "execute transaction", "transaction")?;
    let effects = required(
        transaction.effects,
        "execute transaction",
        "transaction.effects",
    )?;
    let status = required(
        effects.status,
        "execute transaction",
        "transaction.effects.status",
    )?;
    let success = required(
        status.success,
        "execute transaction",
        "transaction.effects.status.success",
    )?;
    if !success {
        let reason = status
            .error
            .and_then(|error| error.description)
            .unwrap_or_else(|| "transaction execution failed".into());
        return Err(SuiError::ExecutionFailed {
            phase: ExecutionPhase::Submission,
            reason,
        });
    }
    let digest = required(
        transaction.digest,
        "execute transaction",
        "transaction.digest",
    )?
    .parse::<Digest>()
    .map_err(|_| {
        invalid(
            "execute transaction",
            "transaction.digest",
            "invalid digest",
        )
    })?
    .to_string();
    if digest != submitted_digest {
        return Err(invalid(
            "execute transaction",
            "transaction.digest",
            format!("expected {submitted_digest}, got {digest}"),
        ));
    }
    Ok(digest)
}
