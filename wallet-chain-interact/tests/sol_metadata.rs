//! Offline RPC regressions. The fixture contains public USDG mint account bytes,
//! fetched from Solana mainnet on 2026-09-16; no live RPC is used by these tests.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use wallet_chain_interact::sol::Provider;
use wallet_transport::client::RpcClient;

const USDG: &str = "2u1tszSeqZ3qBWF3uNGPFc8TzMk2tdiwknnRMWGWjGWH";
const METAPLEX: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";
const LEGACY: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

struct RpcFixture {
    provider: Provider,
    calls: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl RpcFixture {
    fn new(accounts: HashMap<String, Value>, rpc_error: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();
        let worker = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buf = [0; 4096];
                let (body_start, length) = loop {
                    let count = socket.read(&mut buf).unwrap();
                    assert!(count > 0, "incomplete HTTP headers");
                    bytes.extend_from_slice(&buf[..count]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                        let length = headers
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap();
                        break (end + 4, length);
                    }
                };
                while bytes.len() < body_start + length {
                    let count = socket.read(&mut buf).unwrap();
                    assert!(count > 0, "incomplete HTTP body");
                    bytes.extend_from_slice(&buf[..count]);
                }
                let request: Value =
                    serde_json::from_slice(&bytes[body_start..body_start + length]).unwrap();
                assert_eq!(request["method"], "getAccountInfo");
                let address = request["params"][0].as_str().unwrap();
                recorded.lock().unwrap().push(address.to_owned());
                let response = if rpc_error {
                    json!({"jsonrpc":"2.0", "id":request["id"], "error":{"code":-32000,"message":"fixture RPC failure"}})
                } else {
                    json!({"jsonrpc":"2.0", "id":request["id"], "result":{"context":{"slot":1},"value":accounts.get(address).cloned().unwrap_or(Value::Null)}})
                }.to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
        });
        Self {
            provider: Provider::new(
                RpcClient::new(&url, None, Some(Duration::from_secs(10))).unwrap(),
            )
            .unwrap(),
            calls,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for RpcFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let joined = self.worker.take().unwrap().join();
        if !thread::panicking() {
            joined.unwrap();
        }
    }
}
fn mint() -> Pubkey {
    Pubkey::from_str(USDG).unwrap()
}
fn mint_bytes() -> Vec<u8> {
    STANDARD
        .decode(include_str!("fixtures/usdg_mint.base64").trim())
        .unwrap()
}
fn account(owner: &str, bytes: &[u8]) -> Value {
    json!({"owner":owner,"data":[STANDARD.encode(bytes),"base64"],"executable":false,"lamports":1,"rentEpoch":0,"space":bytes.len()})
}
fn metadata_pda() -> String {
    Pubkey::find_program_address(
        &[
            b"metadata",
            Pubkey::from_str(METAPLEX).unwrap().as_ref(),
            mint().as_ref(),
        ],
        &Pubkey::from_str(METAPLEX).unwrap(),
    )
    .0
    .to_string()
}
fn metaplex_bytes(metadata_mint: Pubkey) -> Vec<u8> {
    let mut bytes = vec![4]; // MetadataV1
    bytes.extend([0; 32]);
    bytes.extend(metadata_mint.to_bytes());
    for text in [
        "Global Dollar",
        "USDG",
        "https://example.invalid/token.json",
    ] {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text.as_bytes());
    }
    bytes.extend([0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    bytes
}
fn extension_offset(bytes: &[u8], kind: u16) -> usize {
    let mut offset = 166;
    while offset + 4 <= bytes.len() {
        let tag = u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap());
        let len = u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap()) as usize;
        if tag == kind {
            return offset + 4;
        }
        offset += 4 + len;
    }
    panic!("missing fixture extension {kind}")
}

#[tokio::test]
async fn usdg_token_2022_metadata_without_metaplex() {
    let rpc = RpcFixture::new(
        HashMap::from([(
            USDG.into(),
            account(&spl_token_2022::id().to_string(), &mint_bytes()),
        )]),
        false,
    );
    assert_eq!(rpc.provider.token_symbol(USDG).await.unwrap(), "USDG");
    assert_eq!(
        rpc.provider.token_name(USDG).await.unwrap(),
        "Global Dollar"
    );
    assert!(
        rpc.calls
            .lock()
            .unwrap()
            .iter()
            .all(|address| address == USDG)
    );
}

#[tokio::test]
async fn legacy_metaplex_metadata_remains_supported() {
    let mut bytes = mint_bytes()[..82].to_vec();
    bytes[45] = 1;
    let rpc = RpcFixture::new(
        HashMap::from([
            (USDG.into(), account(LEGACY, &bytes)),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert_eq!(rpc.provider.token_symbol(USDG).await.unwrap(), "USDG");
    assert_eq!(
        rpc.provider.token_name(USDG).await.unwrap(),
        "Global Dollar"
    );
}

#[tokio::test]
async fn missing_metadata_returns_error() {
    let rpc = RpcFixture::new(
        HashMap::from([(USDG.into(), account(LEGACY, &mint_bytes()[..82]))]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn malformed_metaplex_returns_error_without_panicking() {
    let rpc = RpcFixture::new(
        HashMap::from([
            (USDG.into(), account(LEGACY, &mint_bytes()[..82])),
            (metadata_pda(), account(METAPLEX, &[4, 0])),
        ]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn mismatched_metaplex_mint_is_rejected() {
    let rpc = RpcFixture::new(
        HashMap::from([
            (USDG.into(), account(LEGACY, &mint_bytes()[..82])),
            (
                metadata_pda(),
                account(METAPLEX, &metaplex_bytes(Pubkey::new_unique())),
            ),
        ]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn mismatched_token_2022_metadata_mint_is_rejected() {
    let mut bytes = mint_bytes();
    let offset = extension_offset(
        &bytes,
        spl_token_2022::extension::ExtensionType::TokenMetadata as u16,
    );
    bytes[offset + 32..offset + 64].copy_from_slice(Pubkey::new_unique().as_ref());
    let rpc = RpcFixture::new(
        HashMap::from([(
            USDG.into(),
            account(&spl_token_2022::id().to_string(), &bytes),
        )]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn rpc_errors_are_propagated() {
    let rpc = RpcFixture::new(HashMap::new(), true);
    let err = rpc.provider.token_symbol(USDG).await.unwrap_err();
    assert!(err.to_string().contains("fixture RPC failure"), "{err}");
    assert_eq!(rpc.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn token_2022_without_extensions_falls_back_to_metaplex() {
    let rpc = RpcFixture::new(
        HashMap::from([
            (
                USDG.into(),
                account(&spl_token_2022::id().to_string(), &mint_bytes()[..82]),
            ),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert_eq!(rpc.provider.token_symbol(USDG).await.unwrap(), "USDG");
}

#[tokio::test]
async fn external_pointer_to_metaplex_is_supported() {
    let mut bytes = mint_bytes();
    let offset = extension_offset(
        &bytes,
        spl_token_2022::extension::ExtensionType::MetadataPointer as u16,
    );
    bytes[offset + 32..offset + 64]
        .copy_from_slice(Pubkey::from_str(&metadata_pda()).unwrap().as_ref());
    let rpc = RpcFixture::new(
        HashMap::from([
            (
                USDG.into(),
                account(&spl_token_2022::id().to_string(), &bytes),
            ),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert_eq!(
        rpc.provider.token_name(USDG).await.unwrap(),
        "Global Dollar"
    );
    assert!(rpc.calls.lock().unwrap().contains(&metadata_pda()));
}

#[tokio::test]
async fn unsupported_external_pointer_is_not_ignored() {
    let mut bytes = mint_bytes();
    let target = Pubkey::new_unique();
    let offset = extension_offset(
        &bytes,
        spl_token_2022::extension::ExtensionType::MetadataPointer as u16,
    );
    bytes[offset + 32..offset + 64].copy_from_slice(target.as_ref());
    let rpc = RpcFixture::new(
        HashMap::from([
            (
                USDG.into(),
                account(&spl_token_2022::id().to_string(), &bytes),
            ),
            (
                target.to_string(),
                account(&Pubkey::new_unique().to_string(), &[1, 2, 3]),
            ),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn corrupt_token_2022_data_does_not_fall_back() {
    let rpc = RpcFixture::new(
        HashMap::from([
            (
                USDG.into(),
                account(&spl_token_2022::id().to_string(), &[1, 2, 3]),
            ),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
    assert_eq!(rpc.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_metaplex_owner_is_rejected() {
    let rpc = RpcFixture::new(
        HashMap::from([
            (USDG.into(), account(LEGACY, &mint_bytes()[..82])),
            (metadata_pda(), account(LEGACY, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    assert!(rpc.provider.token_symbol(USDG).await.is_err());
}

#[tokio::test]
async fn corrupt_inline_metadata_does_not_fall_back_to_available_metaplex() {
    let mut bytes = mint_bytes();
    let offset = extension_offset(
        &bytes,
        spl_token_2022::extension::ExtensionType::TokenMetadata as u16,
    );
    // Keep the mint and TLV structure valid, but make the Borsh name length
    // exceed the metadata payload. A valid fallback must not mask this error.
    bytes[offset + 64..offset + 68].copy_from_slice(&1000u32.to_le_bytes());
    let rpc = RpcFixture::new(
        HashMap::from([
            (
                USDG.into(),
                account(&spl_token_2022::id().to_string(), &bytes),
            ),
            (metadata_pda(), account(METAPLEX, &metaplex_bytes(mint()))),
        ]),
        false,
    );
    let err = rpc.provider.token_symbol(USDG).await.unwrap_err();
    assert!(
        err.to_string().contains("Invalid Token-2022 metadata"),
        "{err}"
    );
    assert_eq!(*rpc.calls.lock().unwrap(), vec![USDG.to_string()]);
}
