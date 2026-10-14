use super::get_chain;
use wallet_chain_interact::{sui::transfer::TransferOpt, types::ChainPrivateKey};
use wallet_utils::unit;

const USDT: &str = "0xc060006111016b8a020ad5b33834984a437aaa7d3c74c18e09a95d48aceab08c::coin::COIN";

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_block_num() {
    let sui = get_chain();

    let height = sui.block_num().await.unwrap();

    tracing::info!(height, "Sui latest block height");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_balance() {
    let sui = get_chain();

    let addr = "0xca93df9d481ff298080047e612dac1ff537d3e24a843e2608428848a108083ec";
    // let token = Some(SUI_NATIVE_COIN.to_string());
    let token = Some(USDT.to_string());

    let balance = sui.balance(addr, token).await.unwrap();

    tracing::info!(total_balance = balance.to_string(), "Sui balance");
}

#[tokio::test]
#[ignore = "black address test; set SUI_TESTNET_GRPC_URL"]
async fn chain_black_address() {
    let sui = get_chain();

    let coin_type = USDT;
    let owner = "0xca93df9d481ff298080047e612dac1ff537d3e24a843e2608428848a108083ec";

    let black = sui.black_address(coin_type, owner).await.unwrap();

    tracing::info!(black_address = black, "Sui balance");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_query_tx_res() {
    let sui = get_chain();
    let digest = "BVYghmWz5d6PnwTLdnrhb7M9wGhijRmNcfP9CTA3opb1";
    let transaction = sui.query_tx_res(&digest).await.unwrap();

    tracing::info!(transaction = ?transaction, "Sui transaction");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_token_metadata() {
    let sui = get_chain();

    let token = USDT;

    let decimal = sui.decimals(token).await.unwrap();
    tracing::info!(decimal, "Sui token decimal");

    let name = sui.token_name(token).await.unwrap();
    tracing::info!(name, "Sui token name");

    let symbol = sui.token_symbol(token).await.unwrap();
    tracing::info!(symbol, "Sui token symbol");
}

// 预估手续费
#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_estimate() {
    let sui = get_chain();

    let from = "0xca93df9d481ff298080047e612dac1ff537d3e24a843e2608428848a108083ec";
    let to = "0xd0e56a3a99764b0afb4f3e888ff51a65917d481b15aa2b2b059eacd668ebff87";
    let amount = unit::convert_to_u256("0.1", 6).unwrap();
    let token = Some(USDT.to_string());

    let transfer_opt = TransferOpt::new(from, to, amount, token).unwrap();

    let mut select_coin = transfer_opt.select_coin(&sui.provider).await.unwrap();

    let tx = transfer_opt
        .build_pt(&sui.provider, &mut select_coin, None)
        .await
        .unwrap();

    let fee = sui.estimate_fee(from, tx).await.unwrap();

    tracing::info!(fee = ?fee, "Sui estimated fee");
}

// 执行转账
#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn chain_transfer() {
    let sui = get_chain();

    let from = "0xca93df9d481ff298080047e612dac1ff537d3e24a843e2608428848a108083ec";
    let to = "0xd0e56a3a99764b0afb4f3e888ff51a65917d481b15aa2b2b059eacd668ebff87";
    let amount = unit::convert_to_u256("0.1", 6).unwrap();
    let token = Some(USDT.to_string());

    let private_key = ChainPrivateKey::from("");

    let transfer_opt = TransferOpt::new(from, to, amount, token).unwrap();

    let mut select_coin = transfer_opt.select_coin(&sui.provider).await.unwrap();

    let tx = transfer_opt
        .build_pt(&sui.provider, &mut select_coin, None)
        .await
        .unwrap();
    let fee = sui.estimate_fee(from, tx).await.unwrap();

    let tx_data = transfer_opt
        .build_data(&sui.provider, select_coin, fee)
        .await
        .unwrap();

    let fee = sui.exec(tx_data, private_key).await.unwrap();

    tracing::info!(fee = ?fee, "Sui estimated fee");
}
