use crate::ton::{consts::TON_VALUE, errors::TonError};
use num_bigint::BigUint;
use serde::Deserialize;
use tonlib_core::{
    TonAddress,
    cell::{BagOfCells, Cell, EMPTY_ARC_CELL, EitherCellLayout},
    message::{
        HasOpcode as _, JettonTransferMessage, NftOwnershipAssignedMessage, NftTransferMessage,
        TonMessage as _,
    },
    tlb_types::tlb::TLB as _,
};
pub trait GetAddress {
    fn get_address(&self, bounce: bool) -> String;
}
impl GetAddress for String {
    fn get_address(&self, bounce: bool) -> String {
        match TonAddress::from_base64_url(&self) {
            Ok(a) => a.to_base64_url_flags(bounce, false),
            Err(_) => self.clone(),
        }
    }
}

impl GetAddress for AddressId {
    fn get_address(&self, bounce: bool) -> String {
        match TonAddress::from_base64_url(&self.account_address) {
            Ok(a) => a.to_base64_url_flags(bounce, false),
            Err(_) => self.account_address.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawTransaction<T: std::fmt::Debug = String> {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub utime: u64,
    pub data: String,
    pub transaction_id: TransactionId,
    pub fee: String,
    pub storage_fee: String,
    pub other_fee: String,
    pub in_msg: RawMessage<T>,
    pub out_msgs: Vec<RawMessage<T>>,
}

impl<T: std::fmt::Debug> RawTransaction<T> {
    pub fn get_fee(&self) -> crate::Result<f64> {
        let fee = wallet_utils::unit::str_to_num::<f64>(&self.fee)?;
        // let s_fee = wallet_utils::unit::str_to_num::<f64>(&self.storage_fee)?;
        // let o_fee = wallet_utils::unit::str_to_num::<f64>(&self.other_fee)?;

        Ok(fee / TON_VALUE as f64)
    }
}

#[derive(Debug, Deserialize)]
pub struct TransactionId {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub lt: String,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct RawMessage<T: std::fmt::Debug> {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    pub source: T,
    pub destination: T,
    pub value: String,
    pub fwd_fee: String,
    pub ihr_fee: String,
    pub created_lt: String,
    pub body_hash: String,
    pub msg_data: MsgData,
    pub message: Option<String>,
}

// 识别 rawMessage 是什么交易类型
pub enum TxTypes {
    // 代币交易
    JettonTrans,
    // NFT Item 转账
    NftTrans,
    // NFT Item 已向新 owner 发送所有权变更确认
    NftOwnershipAssigned,
    // 普通交易
    Trans,
    // other 交易 目前未识别的交易类型
    Other,
}

impl<T: std::fmt::Debug> RawMessage<T> {
    // 简单验证是否是token交易:目前根据操作码进行判断的,后续估计需要加入地址类型
    pub fn is_token(&self) -> crate::Result<TxTypes> {
        match &self.msg_data {
            MsgData::Raw {
                body,
                init_state: _,
            } => {
                let bag = BagOfCells::parse_base64(&body).map_err(TonError::CellBuild)?;
                let cell = bag
                    .single_root()
                    .map_err(TonError::CellBuild)?
                    .to_cell()
                    .map_err(TonError::CellBuild)?;

                if cell.data().is_empty() {
                    // 没有body 数据 目前也认定为普通交易
                    Ok(TxTypes::Trans)
                } else {
                    let mut parse = cell.parser();
                    let op_code = parse.load_u32(32).map_err(TonError::CellBuild)?;

                    match op_code {
                        // Jetton Transfer
                        0x0f8a7ea5 => Ok(TxTypes::JettonTrans),
                        // NFT Item Transfer (TEP-62)
                        0x5fcc3d14 => Ok(TxTypes::NftTrans),
                        // NFT ownership assigned notification (TEP-62)
                        0x05138d91 => Ok(TxTypes::NftOwnershipAssigned),
                        // Jetton Internal Transfer
                        0x178d4519 => Ok(TxTypes::Other),
                        // Jetton Transfer Notification
                        0x7362d09c => Ok(TxTypes::Other),
                        // Excesses
                        0xd53276db => Ok(TxTypes::Other),
                        // 默认为转账
                        _ => Ok(TxTypes::Trans),
                    }
                }
            }
            // 其他类型都暂时认定为普通的交易类型
            _ => Ok(TxTypes::Trans),
        }
    }

    pub fn parse_token_transfer(&self) -> crate::Result<JettonTransferMessage> {
        match &self.msg_data {
            MsgData::Raw {
                body,
                init_state: _,
            } => {
                let bag = BagOfCells::parse_base64(&body).map_err(TonError::CellBuild)?;
                let cell = bag
                    .single_root()
                    .map_err(TonError::CellBuild)?
                    .to_cell()
                    .map_err(TonError::CellBuild)?;

                // Ok(JettonTransferMessage::parse(&cell).map_err(TonError::TonMsg)?)
                Ok(parse_jetton_message(&cell)?)
            }
            _ => Err(TonError::NotTokenParse("text raw_data ".to_string()))?,
        }
    }

    pub fn parse_nft_transfer(&self) -> crate::Result<NftTransferMessage> {
        match &self.msg_data {
            MsgData::Raw {
                body,
                init_state: _,
            } => {
                let bag = BagOfCells::parse_base64(body).map_err(TonError::CellBuild)?;
                let cell = bag
                    .single_root()
                    .map_err(TonError::CellBuild)?
                    .to_cell()
                    .map_err(TonError::CellBuild)?;

                NftTransferMessage::parse(&cell)
                    .map_err(TonError::TonMsg)
                    .map_err(crate::Error::from)
            }
            _ => Err(TonError::NotTokenParse(
                "NFT transfer requires raw message data".to_string(),
            ))
            .map_err(crate::Error::from),
        }
    }

    pub fn parse_nft_ownership_assigned(&self) -> crate::Result<NftOwnershipAssignedMessage> {
        match &self.msg_data {
            MsgData::Raw {
                body,
                init_state: _,
            } => {
                let bag = BagOfCells::parse_base64(body).map_err(TonError::CellBuild)?;
                let cell = bag
                    .single_root()
                    .map_err(TonError::CellBuild)?
                    .to_cell()
                    .map_err(TonError::CellBuild)?;

                NftOwnershipAssignedMessage::parse(&cell)
                    .map_err(TonError::TonMsg)
                    .map_err(crate::Error::from)
            }
            _ => Err(TonError::NotTokenParse(
                "NFT ownership assignment requires raw message data".to_string(),
            ))
            .map_err(crate::Error::from),
        }
    }

    pub fn value(&self) -> crate::Result<u128> {
        Ok(wallet_utils::unit::str_to_num::<u128>(&self.value)?)
    }
}

impl<T: GetAddress + std::fmt::Debug> RawMessage<T> {
    pub fn get_from(&self, bounce: bool) -> String {
        self.source.get_address(bounce)
    }
    pub fn get_to(&self, bounce: bool) -> String {
        self.destination.get_address(bounce)
    }
}

#[derive(Debug, Deserialize)]
pub struct AddressId {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub account_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "@type")]
pub enum MsgData {
    #[serde(rename = "msg.dataRaw")]
    Raw { body: String, init_state: String },
    #[serde(rename = "msg.dataText")]
    Text { text: String },
    #[serde(rename = "msg.dataEncryptedText")]
    EncryptedText { text: String },
    #[serde(rename = "msg.dataDecryptedText")]
    DecryptedText { text: String },
}

#[derive(Debug, Deserialize)]
pub struct SendBocReturn {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub hash: String,
    #[serde(rename = "@extra")]
    pub extra: String,
}

#[derive(Debug, Deserialize)]
pub struct EstimateFeeResp {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub source_fees: SourceFees,
    pub destination_fees: Vec<DestinationFees>,
    #[serde(rename = "@extra")]
    pub extra: String,
}

impl EstimateFeeResp {
    pub fn get_fee(&self) -> u64 {
        self.source_fees.in_fwd_fee
            + self.source_fees.storage_fee
            + self.source_fees.gas_fee
            + self.source_fees.fwd_fee
    }

    pub fn get_fee_ton(&self) -> f64 {
        self.get_fee() as f64 / TON_VALUE as f64
    }
}

#[derive(Debug, Deserialize)]
pub struct SourceFees {
    #[serde(rename = "@type")]
    pub type_field: String,

    pub in_fwd_fee: u64,
    pub storage_fee: u64,
    pub gas_fee: u64,
    pub fwd_fee: u64,
}

#[derive(Debug, Deserialize)]
pub struct DestinationFees {
    // 如果 destination_fees 不为空，你可以根据实际字段结构补充此结构体
}

// 在某些提交的交易中 ref的标志位不是特别规范,忽略掉forward_payload
fn parse_jetton_message(cell: &Cell) -> Result<JettonTransferMessage, TonError> {
    let mut parser = cell.parser();

    let opcode: u32 = parser.load_u32(32)?;
    let query_id = parser.load_u64(64)?;

    let amount = parser.load_coins()?;
    let destination = parser.load_address()?;
    let response_destination = parser.load_msg_address()?;
    let custom_payload = parser.load_maybe_cell_ref()?;

    let forward_ton_amount = if parser.remaining_bits() > 0 {
        parser.load_coins()?
    } else {
        BigUint::ZERO
    };

    // parser.ensure_empty()?;

    let response_destination = match TonAddress::from_msg_address(response_destination) {
        Ok(addr) => addr,
        Err(_e) => TonAddress::NULL,
    };

    let forward_payload = EMPTY_ARC_CELL.clone();

    let result = JettonTransferMessage {
        query_id,
        amount,
        destination,
        response_destination,
        custom_payload,
        forward_ton_amount,
        forward_payload,
        forward_payload_layout: EitherCellLayout::Native,
    };
    result.verify_opcode(opcode)?;

    Ok(result)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;
    use tonlib_core::{
        cell::EitherCellLayout,
        message::{NftOwnershipAssignedMessage, NftTransferMessage},
    };

    #[test]
    fn nft_ownership_assigned_is_recognized_and_parsed() {
        let prev_owner = TonAddress::from_hex_str(
            "0:286d2c92da998c4fcf82d274257cfa3d0a52bd412ce83dee64a404a7ceaabf31",
        )
        .unwrap();
        let body = NftOwnershipAssignedMessage {
            query_id: 42,
            prev_owner: prev_owner.clone(),
            forward_payload: Arc::new(Cell::default()),
            forward_payload_layout: EitherCellLayout::Native,
        }
        .build()
        .unwrap();
        let body = BagOfCells::from_root(body).serialize(false).unwrap();
        let raw = RawMessage {
            type_field: "raw.message".to_string(),
            hash: "test-hash".to_string(),
            source: "nft-item".to_string(),
            destination: "new-owner".to_string(),
            value: "1".to_string(),
            fwd_fee: "0".to_string(),
            ihr_fee: "0".to_string(),
            created_lt: "0".to_string(),
            body_hash: "body-hash".to_string(),
            msg_data: MsgData::Raw {
                body: wallet_utils::bytes_to_base64(&body),
                init_state: String::new(),
            },
            message: None,
        };

        assert!(matches!(raw.is_token(), Ok(TxTypes::NftOwnershipAssigned)));
        let parsed = raw.parse_nft_ownership_assigned().unwrap();
        assert_eq!(parsed.query_id, 42);
        assert_eq!(parsed.prev_owner, prev_owner);
    }

    #[test]
    fn nft_transfer_is_recognized_and_parsed() {
        let new_owner = TonAddress::from_hex_str(
            "0:71055783d6928e8c007f22f1d799a3b4dbd9034e9d5975364f707b9efe839510",
        )
        .unwrap();
        let response_destination = TonAddress::from_hex_str(
            "0:286d2c92da998c4fcf82d274257cfa3d0a52bd412ce83dee64a404a7ceaabf31",
        )
        .unwrap();
        let body = NftTransferMessage {
            query_id: 42,
            new_owner: new_owner.clone(),
            response_destination: response_destination.clone(),
            custom_payload: None,
            forward_ton_amount: BigUint::from(1u8),
            forward_payload: Arc::new(Cell::default()),
            forward_payload_layout: EitherCellLayout::Native,
        }
        .build()
        .unwrap();
        let body = BagOfCells::from_root(body).serialize(false).unwrap();
        let raw = RawMessage {
            type_field: "raw.message".to_string(),
            hash: "test-hash".to_string(),
            source: "source".to_string(),
            destination: "nft-item".to_string(),
            value: "50000000".to_string(),
            fwd_fee: "0".to_string(),
            ihr_fee: "0".to_string(),
            created_lt: "0".to_string(),
            body_hash: "body-hash".to_string(),
            msg_data: MsgData::Raw {
                body: wallet_utils::bytes_to_base64(&body),
                init_state: String::new(),
            },
            message: None,
        };

        assert!(matches!(raw.is_token(), Ok(TxTypes::NftTrans)));
        let parsed = raw.parse_nft_transfer().unwrap();
        assert_eq!(parsed.query_id, 42);
        assert_eq!(parsed.new_owner, new_owner);
        assert_eq!(parsed.response_destination, response_destination);
    }

    #[test]
    fn test_paras() {
        // let body = "te6cckEBAQEAWQAArQ+KfqUAAAAAAAAAKl6NSlEACAF0bn5pAXYSjoka3kALs7PgDfmJE2xXMvdXGoj1uF6AzwAujc/NIC7CUdEjW8gBdnZ8Ab8xIm2K5l7q41EetwvQGcID8KmiDT0=";
        // let body = "te6cckEBAQEAVwAAqg+KfqX76BmRXBNjPEO5rKAIAWInUylqBJQs4z7SJyZR1usrLYcUS+sgUWKN/ZuGxDv9AA10/ugcep4Gy3heOFSTD83/i7pMVVdLMYR4mRKWvHh/AgL/h0Rw";
        // let body = "te6cckEBAQEAPQAAdQ+KfqUAAAAAAAAAAEE2/AiIAc2wbfbYvxmf/JZY19xuZ130zicwgbqJc2ku+J3CP6hmh3NZQAAAAAAQGxOnyw==";
        let body = "te6cckEBAQEAVgAApw+KfqUAAAAAAAAAAEHc1lAIAf/vjq3HmYxv5DjnWU5FxLoPNEo1hfK+abGs9NtQN5mnADoF7YZAob16c6uyn7OcXcCVTpSsw+UXgAWNsyV0BtUu0MWEoZc=";

        let bag = BagOfCells::parse_base64(&body)
            .map_err(TonError::CellBuild)
            .unwrap();
        let cell = bag
            .single_root()
            .map_err(TonError::CellBuild)
            .unwrap()
            .to_cell()
            .map_err(TonError::CellBuild)
            .unwrap();

        let msg = parse_jetton_message(&cell).unwrap();
        println!("{:#?}", msg);
        // assert!(msg.is_ok());
    }
}
