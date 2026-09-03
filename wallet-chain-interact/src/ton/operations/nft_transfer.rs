use super::BuildInternalMsg;
use crate::ton::provider::Provider;
use crate::ton::{
    address::parse_addr_from_bs64_url, errors::TonError, protocol::account::AddressInformation,
};
use alloy::primitives::U256;
use async_trait::async_trait;
use num_bigint::BigUint;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, EitherCellLayout};
use tonlib_core::message::{
    CommonMsgInfo, InternalMessage, NftTransferMessage, TonMessage as _, TransferMessage,
};
use wallet_types::chain::address::r#type::TonAddressType;

static LAST_QUERY_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug)]
pub struct NftTransferItem {
    pub nft_item: TonAddress,
    pub new_owner: TonAddress,
    pub attached_ton_amount: BigUint,
    pub forward_ton_amount: BigUint,
    pub forward_payload: ArcCell,
}

impl NftTransferItem {
    pub fn new(
        nft_item: &str,
        new_owner: &str,
        attached_ton_amount: U256,
        forward_ton_amount: U256,
    ) -> crate::Result<Self> {
        let attached_ton_amount = BigUint::from_bytes_be(&attached_ton_amount.to_be_bytes::<32>());
        let forward_ton_amount = BigUint::from_bytes_be(&forward_ton_amount.to_be_bytes::<32>());

        if attached_ton_amount == BigUint::from(0u8) || attached_ton_amount <= forward_ton_amount {
            return Err(TonError::InvalidNftAmount {
                attached: attached_ton_amount.to_string(),
                forward: forward_ton_amount.to_string(),
            }
            .into());
        }

        Ok(Self {
            nft_item: parse_addr_from_bs64_url(nft_item)?,
            new_owner: parse_addr_from_bs64_url(new_owner)?,
            attached_ton_amount,
            forward_ton_amount,
            forward_payload: Arc::new(Cell::default()),
        })
    }

    pub fn with_forward_payload(mut self, forward_payload: ArcCell) -> Self {
        self.forward_payload = forward_payload;
        self
    }
}

#[derive(Clone, Debug)]
pub struct NftTransferOpt {
    pub from: TonAddress,
    pub items: Vec<NftTransferItem>,
    query_id: u64,
}

impl NftTransferOpt {
    pub fn new(from: &str, items: Vec<NftTransferItem>) -> crate::Result<Self> {
        Ok(Self {
            from: parse_addr_from_bs64_url(from)?,
            items,
            query_id: next_query_id(),
        })
    }

    pub fn with_query_id(mut self, query_id: u64) -> Self {
        self.query_id = query_id;
        self
    }

    pub fn query_id(&self) -> u64 {
        self.query_id
    }

    fn build_internal_messages(&self, now_time: u32) -> crate::Result<Vec<TransferMessage>> {
        self.items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let query_id =
                    self.query_id
                        .checked_add(index as u64)
                        .ok_or(TonError::QueryIdOverflow {
                            base: self.query_id,
                            index,
                        })?;
                let body = NftTransferMessage {
                    query_id,
                    new_owner: item.new_owner.clone(),
                    response_destination: self.from.clone(),
                    custom_payload: None,
                    forward_ton_amount: item.forward_ton_amount.clone(),
                    forward_payload: item.forward_payload.clone(),
                    forward_payload_layout: EitherCellLayout::Native,
                }
                .build()
                .map_err(TonError::TonMsg)?;

                let internal = InternalMessage {
                    ihr_disabled: true,
                    bounce: true,
                    bounced: false,
                    src: self.from.clone(),
                    dest: item.nft_item.clone(),
                    value: item.attached_ton_amount.clone(),
                    ihr_fee: 0u8.into(),
                    fwd_fee: 0u8.into(),
                    created_lt: 0,
                    created_at: now_time,
                };

                Ok(
                    TransferMessage::new(CommonMsgInfo::InternalMessage(internal))
                        .with_data(body.into())
                        .to_owned(),
                )
            })
            .collect()
    }
}

#[async_trait]
impl BuildInternalMsg for NftTransferOpt {
    async fn build_trans(
        &self,
        address_type: TonAddressType,
        provider: &Provider,
    ) -> crate::Result<Cell> {
        super::validate_message_count(address_type, self.items.len())?;

        let now_time = wallet_utils::time::now().timestamp() as u32;
        let seqno = AddressInformation::seqno(self.from.clone(), provider).await?;
        let transfers = self.build_internal_messages(now_time)?;
        let modes = vec![3; transfers.len()];

        self.build_ext_msgs(transfers, modes, address_type, now_time, seqno)
    }

    fn get_src(&self) -> TonAddress {
        self.from.clone()
    }
}

fn next_query_id() -> u64 {
    let now = wallet_utils::time::now().timestamp_millis().max(0) as u64;
    let previous = LAST_QUERY_ID
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
            Some(now.max(last.saturating_add(1)))
        })
        .expect("query ID update always returns Some");
    now.max(previous.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::{NftTransferItem, NftTransferOpt};
    use crate::ton::{errors::TonError, operations::BuildInternalMsg as _};
    use alloy::primitives::U256;
    use tonlib_core::{
        TonAddress,
        cell::{Cell, CellBuilder},
        message::{
            CommonMsgInfo, HasOpcode as _, NftTransferMessage, TonMessage as _, TransferMessage,
        },
        tlb_types::tlb::TLB as _,
        wallet::versioned::{
            v1_v2::WalletExtMsgBodyV2, v3::WalletExtMsgBodyV3, v4::WalletExtMsgBodyV4,
            v5::WalletExtMsgBodyV5,
        },
    };
    use wallet_types::chain::address::r#type::TonAddressType;

    const FROM: &str = "UQB31xfsF8-mdDIvC-neafiAwGINDGzlM3v3QsVDfqz3s6ks";
    const OWNER_A: &str = "UQDaL1eH_9TU3hceiO7ZsPDEdcmwDhZ0eDZ_NCOIrmjHoSQb";
    const OWNER_B: &str = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs";
    const NFT_A: &str = "EQCxE6mUtQJKFnGfaROTKOt1lZbDiiX1kCixRv7Nw2Id_sDs";
    const NFT_B: &str = "UQDaL1eH_9TU3hceiO7ZsPDEdcmwDhZ0eDZ_NCOIrmjHoSQb";

    fn item(nft: &str, owner: &str) -> NftTransferItem {
        NftTransferItem::new(nft, owner, U256::from(50_000_000u64), U256::from(1u64)).unwrap()
    }

    fn transfer(count: usize) -> NftTransferOpt {
        NftTransferOpt::new(FROM, vec![item(NFT_A, OWNER_A); count])
            .unwrap()
            .with_query_id(10_000)
    }

    fn external_body(
        transfer: &NftTransferOpt,
        address_type: TonAddressType,
    ) -> crate::Result<Cell> {
        let messages = transfer.build_internal_messages(123)?;
        let modes = vec![3; messages.len()];
        transfer.build_ext_msgs(messages, modes, address_type, 123, 7)
    }

    fn sign_v2_body_for_parser(body: &Cell) -> Cell {
        CellBuilder::new()
            .store_slice(&[0; 64])
            .unwrap()
            .store_cell(body)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn nft_item_builds_tep62_internal_message() {
        let payload = CellBuilder::new()
            .store_u32(32, 0)
            .unwrap()
            .store_slice(b"hello")
            .unwrap()
            .build()
            .unwrap()
            .to_arc();
        let first = item(NFT_A, OWNER_A).with_forward_payload(payload.clone());
        let second = item(NFT_B, OWNER_B);
        let transfer = NftTransferOpt::new(FROM, vec![first, second])
            .unwrap()
            .with_query_id(10_000);

        let messages = transfer.build_internal_messages(123).unwrap();
        assert_eq!(messages.len(), 2);

        let expected_from = TonAddress::from_base64_url(FROM).unwrap();
        let expected_nft = TonAddress::from_base64_url(NFT_A).unwrap();
        let expected_owner = TonAddress::from_base64_url(OWNER_A).unwrap();
        match &messages[0].common_msg_info {
            CommonMsgInfo::InternalMessage(internal) => {
                assert!(internal.ihr_disabled);
                assert!(internal.bounce);
                assert!(!internal.bounced);
                assert_eq!(internal.src, expected_from);
                assert_eq!(internal.dest, expected_nft);
                assert_eq!(internal.value, 50_000_000u64.into());
                assert_eq!(internal.created_at, 123);
            }
            other => panic!("expected internal message, got {other:?}"),
        }

        let body = NftTransferMessage::parse(messages[0].data.as_ref().unwrap()).unwrap();
        assert_eq!(NftTransferMessage::opcode(), 0x5fcc3d14);
        assert_eq!(body.query_id, 10_000);
        assert_eq!(body.new_owner, expected_owner);
        assert_eq!(body.response_destination, expected_from);
        assert_eq!(body.forward_ton_amount, 1u64.into());
        assert_eq!(body.forward_payload, payload);

        let second_body = NftTransferMessage::parse(messages[1].data.as_ref().unwrap()).unwrap();
        assert_eq!(second_body.query_id, 10_001);
        assert_eq!(
            second_body.new_owner,
            TonAddress::from_base64_url(OWNER_B).unwrap()
        );
        assert_eq!(
            match &messages[1].common_msg_info {
                CommonMsgInfo::InternalMessage(internal) => internal.dest.clone(),
                other => panic!("expected internal message, got {other:?}"),
            },
            TonAddress::from_base64_url(NFT_B).unwrap()
        );
    }

    #[test]
    fn query_id_is_generated_and_can_be_overridden() {
        let first = NftTransferOpt::new(FROM, vec![item(NFT_A, OWNER_A)]).unwrap();
        let second = NftTransferOpt::new(FROM, vec![item(NFT_A, OWNER_A)]).unwrap();
        assert!(second.query_id() > first.query_id());

        let overridden = transfer(2);
        let messages = overridden.build_internal_messages(123).unwrap();
        let query_ids: Vec<u64> = messages
            .iter()
            .map(|message| {
                NftTransferMessage::parse(message.data.as_ref().unwrap())
                    .unwrap()
                    .query_id
            })
            .collect();
        assert_eq!(query_ids, vec![10_000, 10_001]);

        let overflow = transfer(2).with_query_id(u64::MAX);
        assert!(matches!(
            overflow.build_internal_messages(123),
            Err(crate::Error::TonError(TonError::QueryIdOverflow { .. }))
        ));
    }

    #[test]
    fn invalid_nft_input_is_rejected() {
        assert!(
            NftTransferItem::new(
                "not-an-address",
                OWNER_A,
                U256::from(50_000_000u64),
                U256::from(1u64),
            )
            .is_err()
        );
        assert!(matches!(
            NftTransferItem::new(NFT_A, OWNER_A, U256::ZERO, U256::ZERO),
            Err(crate::Error::TonError(TonError::InvalidNftAmount { .. }))
        ));
        assert!(matches!(
            NftTransferItem::new(NFT_A, OWNER_A, U256::from(1u64), U256::from(1u64),),
            Err(crate::Error::TonError(TonError::InvalidNftAmount { .. }))
        ));
    }

    #[test]
    fn v2_v3_v4_enforce_four_message_limit() {
        for address_type in [
            TonAddressType::V2R1,
            TonAddressType::V2R2,
            TonAddressType::V3R1,
            TonAddressType::V3R2,
            TonAddressType::V4R1,
            TonAddressType::V4R2,
        ] {
            external_body(&transfer(4), address_type).unwrap();
            assert!(matches!(
                external_body(&transfer(5), address_type),
                Err(crate::Error::TonError(TonError::InvalidMessageCount {
                    actual: 5,
                    max: 4,
                }))
            ));
        }

        let v2 = external_body(&transfer(4), TonAddressType::V2R1).unwrap();
        let v2 = WalletExtMsgBodyV2::from_cell(&sign_v2_body_for_parser(&v2)).unwrap();
        assert_eq!(v2.msgs.len(), 4);
        assert_eq!(v2.msgs_modes, vec![3; 4]);

        let v3 = WalletExtMsgBodyV3::from_cell(
            &external_body(&transfer(4), TonAddressType::V3R1).unwrap(),
        )
        .unwrap();
        assert_eq!(v3.msgs.len(), 4);
        assert_eq!(v3.msgs_modes, vec![3; 4]);

        let v4 = WalletExtMsgBodyV4::from_cell(
            &external_body(&transfer(4), TonAddressType::V4R1).unwrap(),
        )
        .unwrap();
        assert_eq!(v4.msgs.len(), 4);
        assert_eq!(v4.msgs_modes, vec![3; 4]);
    }

    #[test]
    fn v5_enforces_255_message_limit() {
        let body = external_body(&transfer(255), TonAddressType::V5R1).unwrap();
        let body = WalletExtMsgBodyV5::from_cell(&body).unwrap();
        assert_eq!(body.msgs.len(), 255);
        assert_eq!(body.msgs_modes, vec![3; 255]);

        assert!(matches!(
            external_body(&transfer(256), TonAddressType::V5R1),
            Err(crate::Error::TonError(TonError::InvalidMessageCount {
                actual: 256,
                max: 255,
            }))
        ));
    }

    #[test]
    fn empty_batch_and_mode_count_mismatch_are_rejected() {
        let empty = NftTransferOpt::new(FROM, vec![]).unwrap();
        assert!(matches!(
            external_body(&empty, TonAddressType::V5R1),
            Err(crate::Error::TonError(TonError::InvalidMessageCount {
                actual: 0,
                max: 255,
            }))
        ));

        let transfer = transfer(2);
        let messages = transfer.build_internal_messages(123).unwrap();
        assert!(matches!(
            transfer.build_ext_msgs(messages, vec![3], TonAddressType::V5R1, 123, 7),
            Err(crate::Error::TonError(TonError::MessageModeCountMismatch {
                messages: 2,
                modes: 1,
            }))
        ));
    }

    #[test]
    fn one_item_uses_the_multi_message_path() {
        let transfer = transfer(1);
        let body = external_body(&transfer, TonAddressType::V5R1).unwrap();
        let body = WalletExtMsgBodyV5::from_cell(&body).unwrap();
        assert_eq!(body.msgs.len(), 1);
        assert_eq!(body.msgs_modes, vec![3]);

        let message = TransferMessage::parse(&body.msgs[0]).unwrap();
        assert!(matches!(
            message.common_msg_info,
            CommonMsgInfo::InternalMessage(_)
        ));
    }
}
