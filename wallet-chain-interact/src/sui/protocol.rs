use super::consts::SUI_VALUE;
use std::collections::BTreeMap;
use sui_sdk_types::ObjectReference;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuiDenyStatus {
    pub current_epoch: bool,
    pub next_epoch: bool,
}

impl SuiDenyStatus {
    pub fn is_blacklisted(self) -> bool {
        self.current_epoch || self.next_epoch
    }

    pub fn cannot_send(self) -> bool {
        self.next_epoch
    }

    pub fn cannot_receive(self) -> bool {
        self.current_epoch
    }
}

/// Total balance includes both Coin objects and Address Balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiBalance {
    pub total_balance: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiCoinMetadata {
    pub decimals: u8,
    pub symbol: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiCoin {
    pub object_ref: ObjectReference,
    pub balance: u64,
}

impl SuiCoin {
    pub fn object_ref(&self) -> ObjectReference {
        self.object_ref.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiOwnedObject {
    pub object_ref: ObjectReference,
    pub object_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiOwnedObjectsPage {
    pub data: Vec<SuiOwnedObject>,
    pub next_cursor: Option<Vec<u8>>,
}

/// Checkpoint identity and the ordered transaction digests needed by scanners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointResult {
    pub sequence_number: u64,
    pub digest: String,
    pub transactions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiObject {
    pub object_ref: ObjectReference,
}

/// Signed balance delta in the coin's smallest unit, including gas for SUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiBalanceChange {
    pub address: String,
    pub coin_type: String,
    pub amount: i128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiQueriedTransaction {
    pub digest: String,
    pub checkpoint: u64,
    pub timestamp_seconds: u64,
    pub success: bool,
    /// Computation plus storage cost minus storage rebate, in MIST.
    pub net_gas_usage: i64,
    pub gas_price: u64,
    /// Owner of the gas payment, which can differ from the transaction sender.
    pub gas_owner: String,
    pub balance_changes: Vec<SuiBalanceChange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiSimulation {
    pub success: bool,
    pub error: Option<String>,
    pub computation_cost: u64,
    pub storage_cost: u64,
}

/// Package modules from gRPC, with bytecode when supplied by the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiPackage {
    pub storage_id: String,
    pub original_id: String,
    pub version: u64,
    pub modules: BTreeMap<String, SuiPackageModule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiPackageModule {
    /// Serialized Move module bytecode, when supplied by the service.
    /// MovePackageService.GetPackage may omit this field.
    pub bytecode: Option<Vec<u8>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct EstimateFeeResp {
    pub gas_fee: u64,
    pub gas_price: u64,
}

impl EstimateFeeResp {
    pub fn new(gas_fee: u64, gas_price: u64) -> Self {
        Self { gas_fee, gas_price }
    }

    // 费用浮动了0.1
    pub fn get_fee(&self) -> u64 {
        // self.gas_fee + (self.gas_fee as f64 * 0.1) as u64
        self.gas_fee
    }

    pub fn get_fee_f64(&self) -> f64 {
        self.get_fee() as f64 / SUI_VALUE
    }
}

pub struct GasObject {
    pub coin: Vec<SuiCoin>,
}
impl GasObject {
    pub fn new(coin: Vec<SuiCoin>) -> Self {
        Self { coin }
    }
}
