use super::{errors::TonError, provider::Provider};
use async_trait::async_trait;
use std::sync::Arc;
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, Cell, TonCellError};
use tonlib_core::message::{TonMessage as _, TransferMessage};
use tonlib_core::tlb_types::tlb::TLB as _;
use tonlib_core::wallet::versioned::{
    DEFAULT_WALLET_ID, DEFAULT_WALLET_ID_V5R1, v1_v2::WalletExtMsgBodyV2, v3::WalletExtMsgBodyV3,
    v4::WalletExtMsgBodyV4, v5::WalletExtMsgBodyV5,
};
use tonlib_core::wallet::wallet_version::WalletVersion;
use wallet_types::chain::address::r#type::TonAddressType;

pub mod nft_transfer;
pub mod token_transfer;
pub mod transfer;

#[async_trait]
pub trait BuildInternalMsg {
    async fn build_trans(
        &self,
        address_type: TonAddressType,
        provider: &Provider,
    ) -> crate::Result<Cell>;

    fn get_src(&self) -> TonAddress;

    fn build_ext_msg(
        &self,
        trans: TransferMessage,
        address_type: TonAddressType,
        now_time: u32,
        seqno: u32,
        spend_all: bool,
    ) -> crate::Result<Cell> {
        let msg_mode = if spend_all { 144 } else { 3 };
        self.build_ext_msgs(vec![trans], vec![msg_mode], address_type, now_time, seqno)
    }

    fn build_ext_msgs(
        &self,
        transfers: Vec<TransferMessage>,
        msg_modes: Vec<u8>,
        address_type: TonAddressType,
        now_time: u32,
        seqno: u32,
    ) -> crate::Result<Cell> {
        validate_message_count(address_type, transfers.len()).map_err(crate::Error::from)?;
        if msg_modes.len() != transfers.len() {
            return Err(TonError::MessageModeCountMismatch {
                messages: transfers.len(),
                modes: msg_modes.len(),
            }
            .into());
        }

        let version = address_type.to_version();
        let msgs_refs = transfers
            .into_iter()
            .map(|trans| trans.build().map(Arc::new).map_err(TonError::TonMsg))
            .collect::<Result<Vec<_>, _>>()?;

        build_ext_msgs(
            version,
            now_time + 60,
            seqno,
            wallet_id_for_version(version),
            msgs_refs,
            msg_modes,
        )
        .map_err(TonError::CellBuild)
        .map_err(crate::Error::from)
    }
}

// 每种钱包类型最大的类型
pub(crate) fn max_messages(address_type: TonAddressType) -> usize {
    match address_type {
        TonAddressType::V2R1
        | TonAddressType::V2R2
        | TonAddressType::V3R1
        | TonAddressType::V3R2
        | TonAddressType::V4R1
        | TonAddressType::V4R2 => 4,
        TonAddressType::V5R1 => 255,
    }
}

pub(crate) fn validate_message_count(
    address_type: TonAddressType,
    actual: usize,
) -> Result<(), TonError> {
    let max = max_messages(address_type);
    if actual == 0 || actual > max {
        return Err(TonError::InvalidMessageCount { actual, max });
    }
    Ok(())
}

fn wallet_id_for_version(version: WalletVersion) -> i32 {
    match version {
        WalletVersion::V5R1 => DEFAULT_WALLET_ID_V5R1,
        _ => DEFAULT_WALLET_ID,
    }
}

fn build_ext_msgs<T: AsRef<[ArcCell]>>(
    version: WalletVersion,
    valid_until: u32,
    msg_seqno: u32,
    wallet_id: i32,
    msgs_refs: T,
    msgs_modes: Vec<u8>,
) -> Result<Cell, TonCellError> {
    let msgs: Vec<ArcCell> = msgs_refs.as_ref().to_vec();

    match version {
        WalletVersion::V2R1 | WalletVersion::V2R2 => WalletExtMsgBodyV2 {
            msg_seqno,
            valid_until,
            msgs_modes,
            msgs,
        }
        .to_cell(),
        WalletVersion::V3R1 | WalletVersion::V3R2 => WalletExtMsgBodyV3 {
            subwallet_id: wallet_id,
            valid_until,
            msg_seqno,
            msgs_modes,
            msgs,
        }
        .to_cell(),
        WalletVersion::V4R1 | WalletVersion::V4R2 => WalletExtMsgBodyV4 {
            subwallet_id: wallet_id,
            valid_until,
            msg_seqno,
            opcode: 0,
            msgs_modes,
            msgs,
        }
        .to_cell(),
        WalletVersion::V5R1 => WalletExtMsgBodyV5 {
            wallet_id,
            valid_until,
            msg_seqno,
            msgs_modes,
            msgs,
        }
        .to_cell(),
        unsupported => Err(TonCellError::InternalError(format!(
            "build_ext_msg for {unsupported:?} is unsupported"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_ext_msgs, wallet_id_for_version};
    use tonlib_core::cell::{ArcCell, Cell, CellBuilder};
    use tonlib_core::tlb_types::tlb::TLB as _;
    use tonlib_core::wallet::versioned::{
        DEFAULT_WALLET_ID, DEFAULT_WALLET_ID_V5R1, v1_v2::WalletExtMsgBodyV2,
        v3::WalletExtMsgBodyV3, v4::WalletExtMsgBodyV4, v5::WalletExtMsgBodyV5,
    };
    use tonlib_core::wallet::wallet_version::WalletVersion;

    const VALID_UNTIL: u32 = 100;
    const SEQNO: u32 = 7;

    fn internal_message() -> ArcCell {
        CellBuilder::new().build().unwrap().to_arc()
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
    fn test_build_v2_external_message_layout() {
        for version in [WalletVersion::V2R1, WalletVersion::V2R2] {
            let cell = build_ext_msgs(
                version,
                VALID_UNTIL,
                SEQNO,
                wallet_id_for_version(version),
                [internal_message()],
                vec![3],
            )
            .unwrap();
            let signed = sign_v2_body_for_parser(&cell);
            let body = WalletExtMsgBodyV2::from_cell(&signed).unwrap();

            assert_eq!(body.valid_until, VALID_UNTIL);
            assert_eq!(body.msg_seqno, SEQNO);
            assert_eq!(body.msgs_modes, vec![3]);
            assert_eq!(body.msgs.len(), 1);
        }
    }

    #[test]
    fn test_build_v3_external_message_layout() {
        for version in [WalletVersion::V3R1, WalletVersion::V3R2] {
            let cell = build_ext_msgs(
                version,
                VALID_UNTIL,
                SEQNO,
                wallet_id_for_version(version),
                [internal_message()],
                vec![3],
            )
            .unwrap();
            let body = WalletExtMsgBodyV3::from_cell(&cell).unwrap();

            assert_eq!(body.subwallet_id, DEFAULT_WALLET_ID);
            assert_eq!(body.valid_until, VALID_UNTIL);
            assert_eq!(body.msg_seqno, SEQNO);
            assert_eq!(body.msgs_modes, vec![3]);
        }
    }

    #[test]
    fn test_build_v4_external_message_layout_and_spend_all_mode() {
        for version in [WalletVersion::V4R1, WalletVersion::V4R2] {
            let cell = build_ext_msgs(
                version,
                VALID_UNTIL,
                SEQNO,
                wallet_id_for_version(version),
                [internal_message()],
                vec![144],
            )
            .unwrap();
            let body = WalletExtMsgBodyV4::from_cell(&cell).unwrap();

            assert_eq!(body.subwallet_id, DEFAULT_WALLET_ID);
            assert_eq!(body.valid_until, VALID_UNTIL);
            assert_eq!(body.msg_seqno, SEQNO);
            assert_eq!(body.opcode, 0);
            assert_eq!(body.msgs_modes, vec![144]);
        }
    }

    #[test]
    fn test_build_v5_external_message_uses_v5_wallet_id() {
        let version = WalletVersion::V5R1;
        let cell = build_ext_msgs(
            version,
            VALID_UNTIL,
            SEQNO,
            wallet_id_for_version(version),
            [internal_message()],
            vec![3],
        )
        .unwrap();
        let body = WalletExtMsgBodyV5::from_cell(&cell).unwrap();

        assert_eq!(body.wallet_id, DEFAULT_WALLET_ID_V5R1);
        assert_eq!(body.valid_until, VALID_UNTIL);
        assert_eq!(body.msg_seqno, SEQNO);
        assert_eq!(body.msgs_modes, vec![3]);
    }
}
