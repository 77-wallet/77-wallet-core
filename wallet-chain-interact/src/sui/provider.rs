use super::error::SuiError;
use super::protocol::{
    CheckpointResult, SuiBalance, SuiCoin, SuiCoinMetadata, SuiDenyStatus, SuiObject,
    SuiOwnedObject, SuiOwnedObjectsPage, SuiPackage, SuiQueriedTransaction, SuiSimulation,
};
use std::collections::{HashMap, HashSet};
use sui_rpc::Client as GrpcClient;
use sui_rpc::field::{FieldMask, FieldMaskUtil};
use sui_rpc::proto::sui::rpc::v2::{
    BatchGetTransactionsRequest, GetBalanceRequest, GetCoinInfoRequest, GetEpochRequest,
    GetObjectRequest, GetPackageRequest, GetServiceInfoRequest, GetTransactionRequest,
    ListOwnedObjectsRequest, ListOwnedObjectsResponse, SimulateTransactionRequest,
};
use sui_sdk_types::{Address, Digest, ProgrammableTransaction, Transaction};
use tonic::metadata::{Ascii, MetadataKey, MetadataValue};

mod mapping;
mod request;
#[cfg(test)]
mod tests;
use mapping::{
    map_checkpoint, map_checkpoint_height, map_execution_result, map_object_response, map_package,
    map_reference_gas_price, map_simulation, map_transaction_batch, map_transaction_response,
};
use request::{
    checkpoint_request, coin_list_request, execution_request, full_simulation_request,
    kind_simulation_request, parse_object_type_filter, transaction_read_mask,
};

pub struct Provider {
    pub grpc_client: GrpcClient,
}

fn rpc_error(operation: &'static str, status: tonic::Status) -> crate::Error {
    SuiError::transport(operation, status).into()
}

fn resource_error(
    operation: &'static str,
    resource: &'static str,
    id: &str,
    status: tonic::Status,
) -> crate::Error {
    if status.code() == tonic::Code::NotFound {
        SuiError::NotFound {
            resource,
            id: id.to_string(),
        }
        .into()
    } else {
        rpc_error(operation, status)
    }
}

fn submission_error(digest: String, status: tonic::Status) -> crate::Error {
    match status.code() {
        tonic::Code::Cancelled
        | tonic::Code::Unknown
        | tonic::Code::DeadlineExceeded
        | tonic::Code::ResourceExhausted
        | tonic::Code::Aborted
        | tonic::Code::Internal
        | tonic::Code::Unavailable => SuiError::SubmissionOutcomeUnknown {
            digest: Some(digest),
            code: status.code(),
            message: status.message().to_string(),
        },
        _ => SuiError::transport("execute transaction", status),
    }
    .into()
}

fn parse_address(value: &str, field: &'static str) -> crate::Result<Address> {
    wallet_utils::address::parse_sui_address(value).map_err(|error| {
        SuiError::InvalidInput {
            field,
            reason: error.to_string(),
        }
        .into()
    })
}

fn parse_digest(value: &str, field: &'static str) -> crate::Result<String> {
    value
        .parse::<Digest>()
        .map(|digest| digest.to_string())
        .map_err(|error| {
            SuiError::InvalidInput {
                field,
                reason: error.to_string(),
            }
            .into()
        })
}

fn collect_coin_page(
    response: ListOwnedObjectsResponse,
    seen_tokens: &mut HashSet<Vec<u8>>,
    coins: &mut Vec<SuiCoin>,
) -> crate::Result<Option<Vec<u8>>> {
    let next = response.next_page_token.map(|token| token.to_vec());
    if let Some(token) = &next {
        if token.is_empty() || !seen_tokens.insert(token.clone()) {
            return Err(SuiError::invalid_response(
                "list owned coins",
                "next_page_token",
                "repeated or empty token",
            )
            .into());
        }
    }
    for object in response.objects {
        coins.push(object.try_into()?);
    }
    Ok(next)
}

impl Provider {
    pub async fn coin_deny_status(
        &self,
        coin_type: &str,
        owner: &str,
    ) -> crate::Result<SuiDenyStatus> {
        let coin_type = wallet_utils::address::parse_sui_type_tag(coin_type)?;
        let owner = parse_address(owner, "owner")?;

        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_object(request::deny_list_object_request())
            .await
            .map_err(|status| {
                resource_error(
                    "get deny-list object",
                    "deny-list object",
                    request::DENY_LIST_OBJECT_ID,
                    status,
                )
            })?
            .into_inner();
        let version = mapping::map_deny_list_version(response)?;
        let tx = request::deny_list_transaction(coin_type, owner, version)?;
        let request = request::deny_list_simulation_request(owner, tx);
        let response = client
            .execution_client()
            .simulate_transaction(request)
            .await
            .map_err(|status| rpc_error("simulate deny-list query", status))?
            .into_inner();
        Ok(mapping::map_deny_status(response)?)
    }

    pub async fn black_address(&self, coin_type: &str, owner: &str) -> crate::Result<bool> {
        Ok(self
            .coin_deny_status(coin_type, owner)
            .await?
            .is_blacklisted())
    }

    /// Configures a lazy full-node gRPC connection.
    /// Optional headers are caller-defined ASCII gRPC metadata.
    /// Must be called inside a Tokio runtime.
    pub fn new(
        full_node_url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> crate::Result<Self> {
        let mut grpc_client =
            GrpcClient::new(full_node_url).map_err(|error| SuiError::InvalidInput {
                field: "full_node_url",
                reason: error.to_string(),
            })?;

        if let Some(headers) = headers {
            let mut interceptor = sui_rpc::client::HeadersInterceptor::new();
            for (name, value) in headers {
                let name =
                    name.parse::<MetadataKey<Ascii>>()
                        .map_err(|error| SuiError::InvalidInput {
                            field: "headers",
                            reason: error.to_string(),
                        })?;
                let value = value.parse::<MetadataValue<Ascii>>().map_err(|error| {
                    SuiError::InvalidInput {
                        field: "headers",
                        reason: error.to_string(),
                    }
                })?;
                interceptor.headers_mut().insert(name, value);
            }
            grpc_client = grpc_client.with_headers(interceptor);
        }

        Ok(Self { grpc_client })
    }

    pub async fn balance(&self, addr: &str, coin_type: &str) -> crate::Result<SuiBalance> {
        let parsed_addr = parse_address(addr, "owner")?;
        let mut client = self.grpc_client.clone();
        let mut request = GetBalanceRequest::default();
        request.owner = Some(parsed_addr.to_string());
        request.coin_type = Some(coin_type.to_string());

        let response = client
            .state_client()
            .get_balance(request)
            .await
            .map_err(|status| rpc_error("get balance", status))?
            .into_inner();
        Ok(response.try_into()?)
    }

    pub async fn latest_block(&self) -> crate::Result<String> {
        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_service_info(GetServiceInfoRequest::default())
            .await
            .map_err(|status| rpc_error("get service info", status))?
            .into_inner();

        Ok(map_checkpoint_height(response)?.to_string())
    }

    pub async fn query_tx_info(&self, digest: &str) -> crate::Result<SuiQueriedTransaction> {
        let digest = parse_digest(digest, "transaction_digest")?;
        let mut request = GetTransactionRequest::default();
        request.digest = Some(digest.clone());
        request.read_mask = Some(transaction_read_mask());

        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_transaction(request)
            .await
            .map_err(|status| resource_error("get transaction", "transaction", &digest, status))?
            .into_inner();
        Ok(map_transaction_response(response, &digest)?)
    }

    pub async fn get_coin_metadata(&self, coin_type: &str) -> crate::Result<SuiCoinMetadata> {
        let mut client = self.grpc_client.clone();
        let mut request = GetCoinInfoRequest::default();
        request.coin_type = Some(coin_type.to_string());

        let response = client
            .state_client()
            .get_coin_info(request)
            .await
            .map_err(|status| resource_error("get coin info", "coin metadata", coin_type, status))?
            .into_inner();
        Ok(response.try_into()?)
    }

    pub async fn get_reference_gas_price(&self) -> crate::Result<u64> {
        let mut request = GetEpochRequest::default();
        request.read_mask = Some(FieldMask::from_paths(["reference_gas_price"]));
        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_epoch(request)
            .await
            .map_err(|status| rpc_error("get epoch", status))?
            .into_inner();
        Ok(map_reference_gas_price(response)?)
    }

    pub async fn get_owned_objects(
        &self,
        addr: &str,
        filter: Option<serde_json::Value>,
        cursor: Option<Vec<u8>>,
        limit: Option<u32>,
    ) -> crate::Result<SuiOwnedObjectsPage> {
        let object_type = parse_object_type_filter(filter)?;
        let parsed_addr = parse_address(addr, "owner")?;
        let mut client = self.grpc_client.clone();
        let mut request = ListOwnedObjectsRequest::default();
        request.owner = Some(parsed_addr.to_string());
        request.object_type = object_type;
        request.page_token = cursor.map(Into::into);
        request.page_size = limit;
        request.read_mask = Some(FieldMask::from_paths([
            "object_id",
            "version",
            "digest",
            "object_type",
        ]));
        let response = client
            .state_client()
            .list_owned_objects(request)
            .await
            .map_err(|status| rpc_error("list owned objects", status))?
            .into_inner();
        let data = response
            .objects
            .into_iter()
            .map(|object| SuiOwnedObject::try_from(object).map_err(Into::into))
            .collect::<crate::Result<Vec<_>>>()?;
        Ok(SuiOwnedObjectsPage {
            data,
            next_cursor: response.next_page_token.map(|token| token.to_vec()),
        })
    }

    pub async fn get_object_by_id(&self, id: &str) -> crate::Result<SuiObject> {
        let id = id
            .parse::<Address>()
            .map_err(|error| SuiError::InvalidInput {
                field: "object_id",
                reason: error.to_string(),
            })?;
        let mut request = GetObjectRequest::default();
        request.object_id = Some(id.to_string());
        request.read_mask = Some(FieldMask::from_paths(["object_id", "version", "digest"]));
        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_object(request)
            .await
            .map_err(|status| resource_error("get object", "object", &id.to_string(), status))?
            .into_inner();
        Ok(map_object_response(response, &id.to_string())?)
    }

    pub async fn get_all_coins_by_owner(
        &self,
        addr: &str,
        coin_type: &str,
    ) -> crate::Result<Vec<SuiCoin>> {
        let parsed_addr = parse_address(addr, "owner")?;
        let mut client = self.grpc_client.clone();
        let mut state = client.state_client();
        let mut cursor = None;
        let mut seen_tokens = HashSet::new();
        let mut all_coins = Vec::new();

        loop {
            let request = coin_list_request(&parsed_addr.to_string(), coin_type, cursor);
            let page = state
                .list_owned_objects(request)
                .await
                .map_err(|status| rpc_error("list owned coins", status))?
                .into_inner();
            cursor = collect_coin_page(page, &mut seen_tokens, &mut all_coins)?;
            if cursor.is_none() {
                break;
            }
        }

        Ok(all_coins)
    }

    pub async fn dev_inspect_transaction(
        &self,
        sender: &str,
        tx: ProgrammableTransaction,
        gas_price: u64,
    ) -> crate::Result<SuiSimulation> {
        let request = kind_simulation_request(sender, tx, gas_price)?;
        self.simulate_transaction(request).await
    }

    pub async fn dry_run_transaction(&self, tx_data: &Transaction) -> crate::Result<SuiSimulation> {
        let request = full_simulation_request(tx_data)?;
        self.simulate_transaction(request).await
    }

    async fn simulate_transaction(
        &self,
        request: SimulateTransactionRequest,
    ) -> crate::Result<SuiSimulation> {
        let mut client = self.grpc_client.clone();
        let response = client
            .execution_client()
            .simulate_transaction(request)
            .await
            .map_err(|status| rpc_error("simulate transaction", status))?
            .into_inner();
        Ok(map_simulation(response)?)
    }

    pub async fn get_check_point(&self, check_point: &str) -> crate::Result<CheckpointResult> {
        let request = checkpoint_request(check_point)?;
        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .get_checkpoint(request)
            .await
            .map_err(|status| resource_error("get checkpoint", "checkpoint", check_point, status))?
            .into_inner();
        Ok(map_checkpoint(response, check_point)?)
    }

    pub async fn get_multi_trans(
        &self,
        digests: &[String],
    ) -> crate::Result<Vec<SuiQueriedTransaction>> {
        if digests.is_empty() {
            return Ok(Vec::new());
        }
        let digests = digests
            .iter()
            .map(|digest| parse_digest(digest, "transaction_digest"))
            .collect::<crate::Result<Vec<_>>>()?;
        let mut request = BatchGetTransactionsRequest::default();
        request.digests = digests.clone();
        request.read_mask = Some(transaction_read_mask());
        let mut client = self.grpc_client.clone();
        let response = client
            .ledger_client()
            .batch_get_transactions(request)
            .await
            .map_err(|status| rpc_error("batch get transactions", status))?
            .into_inner();
        Ok(map_transaction_batch(response, &digests)?)
    }

    pub async fn send_transaction(
        &self,
        tx_bytes: Vec<u8>,
        signatures: Vec<Vec<u8>>,
    ) -> crate::Result<String> {
        let (request, digest) = execution_request(tx_bytes, signatures)?;
        let mut client = self.grpc_client.clone();
        let response = client
            .execution_client()
            .execute_transaction(request)
            .await
            .map_err(|status| submission_error(digest.clone(), status))?
            .into_inner();
        Ok(map_execution_result(response, &digest)?)
    }

    /// Returns package modules with optional bytecode supplied by the gRPC service.
    pub async fn get_package_modules(&self, package_id: &str) -> crate::Result<SuiPackage> {
        let package_id = package_id
            .parse::<Address>()
            .map_err(|error| SuiError::InvalidInput {
                field: "package_id",
                reason: error.to_string(),
            })?;
        let mut request = GetPackageRequest::default();
        request.package_id = Some(package_id.to_string());
        let mut client = self.grpc_client.clone();
        let response = client
            .package_client()
            .get_package(request)
            .await
            .map_err(|status| {
                resource_error("get package", "package", &package_id.to_string(), status)
            })?
            .into_inner();
        Ok(map_package(response, package_id)?)
    }
}
