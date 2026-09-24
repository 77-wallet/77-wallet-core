use crate::get_provider;
use sui_sdk_types::{
    Address, Argument, Command, Digest, GasPayment, ProgrammableTransaction, SplitCoins,
    Transaction, TransactionExpiration, TransactionKind, TransferObjects,
};
use wallet_chain_interact::sui::{
    builder::ProgrammableTransactionBuilder, consts::SUI_NATIVE_COIN,
};

const TEST_ADDRESS: &str = "0x4c1cd48f7f203870be350d7a18c5a827131cecc7322b1571b9a69aeae7dda5f2";

// Simulate a one-MIST transfer back to the sender without submitting it.
fn simulation_transaction(sender: Address) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    let amount = builder.pure(1u64).unwrap();
    builder
        .command(Command::SplitCoins(SplitCoins {
            coin: Argument::Gas,
            amounts: vec![amount],
        }))
        .unwrap();
    let recipient = builder.pure(sender).unwrap();
    builder
        .command(Command::TransferObjects(TransferObjects {
            objects: vec![Argument::NestedResult(0, 0)],
            address: recipient,
        }))
        .unwrap();
    builder.finish()
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_reference_gas_price() {
    let provider = get_provider();

    let price = provider.get_reference_gas_price().await.unwrap();

    tracing::info!(price = ?price, "Sui reference gas price");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_owned_objects() {
    let provider = get_provider();
    let owner = TEST_ADDRESS;
    let page = provider
        .get_owned_objects(owner, None, None, Some(10))
        .await
        .unwrap();

    assert!(page.data.len() <= 10);
    for object in &page.data {
        assert!(object.object_ref.version() > 0);
        assert!(!object.object_type.is_empty());
    }
    if let Some(cursor) = &page.next_cursor {
        assert!(!cursor.is_empty());
        let next_page = provider
            .get_owned_objects(owner, None, Some(cursor.clone()), Some(10))
            .await
            .unwrap();
        assert!(next_page.data.len() <= 10);
        assert_ne!(next_page.next_cursor.as_ref(), Some(cursor));
        for object in &next_page.data {
            assert!(page.data.iter().all(|previous| {
                previous.object_ref.object_id() != object.object_ref.object_id()
            }));
        }
        tracing::info!(?next_page, "Sui next owned objects page");
    }
    tracing::info!(?page, "Sui owned objects");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_object_by_id() {
    let provider = get_provider();
    // The shared system-state object is available without a user-owned fixture.
    let id = "0x5";
    let object = provider.get_object_by_id(id).await.unwrap();

    assert_eq!(
        *object.object_ref.object_id(),
        id.parse::<Address>().unwrap()
    );
    assert!(object.object_ref.version() > 0);
    tracing::info!(?object, "Sui object");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_all_coins_by_owner() {
    let provider = get_provider();
    let coins = provider
        .get_all_coins_by_owner(TEST_ADDRESS, SUI_NATIVE_COIN)
        .await
        .unwrap();

    let mut ids = std::collections::HashSet::new();
    for coin in &coins {
        assert!(coin.object_ref.version() > 0);
        assert!(
            ids.insert(*coin.object_ref.object_id()),
            "duplicate coin object"
        );
    }
    // An address with no Coin objects legitimately returns an empty list.
    tracing::info!(?coins, "Sui owned coins");
}

#[tokio::test]
#[ignore = "manual simulation; configure get_provider and fund TEST_ADDRESS"]
async fn provider_dev_inspect_transaction() {
    let provider = get_provider();
    let sender = TEST_ADDRESS;
    let tx = simulation_transaction(sender.parse().unwrap());
    let gas_price = provider.get_reference_gas_price().await.unwrap();
    let simulation = provider
        .dev_inspect_transaction(sender, tx, gas_price)
        .await
        .unwrap();

    assert!(
        simulation.success,
        "simulation failed: {:?}",
        simulation.error
    );
    assert!(simulation.error.is_none());
    assert!(simulation.computation_cost > 0);
    tracing::info!(?simulation, "Sui dev inspect result");
}

#[tokio::test]
#[ignore = "manual simulation; configure get_provider and fund TEST_ADDRESS"]
async fn provider_dry_run_transaction() {
    let provider = get_provider();
    let sender = TEST_ADDRESS;
    let address = sender.parse::<Address>().unwrap();
    let budget = 10_000_000u64;
    let coins = provider
        .get_all_coins_by_owner(sender, SUI_NATIVE_COIN)
        .await
        .unwrap();
    let gas_coin = coins
        .iter()
        .find(|coin| coin.balance > budget)
        .expect("TEST_ADDRESS needs a SUI Coin object with more than 0.01 SUI");
    let gas_price = provider.get_reference_gas_price().await.unwrap();
    let tx = Transaction {
        kind: TransactionKind::ProgrammableTransaction(simulation_transaction(address)),
        sender: address,
        gas_payment: GasPayment {
            objects: vec![gas_coin.object_ref()],
            owner: address,
            price: gas_price,
            budget,
        },
        expiration: TransactionExpiration::None,
    };
    let simulation = provider.dry_run_transaction(&tx).await.unwrap();

    assert!(
        simulation.success,
        "simulation failed: {:?}",
        simulation.error
    );
    assert!(simulation.error.is_none());
    assert!(simulation.computation_cost > 0);
    tracing::info!(?simulation, "Sui dry run result");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_checkpoint() {
    let provider = get_provider();
    let id = match std::env::var("SUI_TEST_CHECKPOINT") {
        Ok(id) => id,
        Err(_) => provider.latest_block().await.unwrap(),
    };
    let checkpoint = provider.get_check_point(&id).await.unwrap();

    if let Ok(sequence) = id.parse::<u64>() {
        assert_eq!(checkpoint.sequence_number, sequence);
    } else {
        assert_eq!(checkpoint.digest, id);
    }
    checkpoint.digest.parse::<Digest>().unwrap();
    tracing::info!(?checkpoint, "Sui checkpoint");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL and SUI_TEST_TX_DIGESTS"]
async fn provider_batch_get_transactions() {
    let provider = get_provider();
    // Supply one or more existing digests separated by commas.

    let digests = vec!["J53ktfRz3sGur4sDZkhtCJtFHvXVEboSwKCbGqzQNBbK".to_string()];

    let transactions = provider.get_multi_trans(&digests).await.unwrap();

    assert_eq!(transactions.len(), digests.len());
    for (transaction, digest) in transactions.iter().zip(&digests) {
        assert_eq!(&transaction.digest, digest);
    }
    tracing::info!(?transactions, "Sui batch transactions");
}

#[tokio::test]
#[ignore = "manual gRPC test; set SUI_TESTNET_GRPC_URL"]
async fn provider_get_package_modules() {
    let provider = get_provider();
    let id = "0x2";
    let package = provider.get_package_modules(id).await.unwrap();

    assert_eq!(
        package.storage_id,
        id.parse::<Address>().unwrap().to_string()
    );
    package.original_id.parse::<Address>().unwrap();
    assert!(package.version > 0);
    assert!(!package.modules.is_empty());
    for (name, module) in &package.modules {
        assert!(!name.is_empty());
        if let Some(bytecode) = &module.bytecode {
            assert!(!bytecode.is_empty());
        }
    }
    tracing::info!(package_id = %package.storage_id, modules = ?package.modules.keys().collect::<Vec<_>>(), "Sui package modules");
}
