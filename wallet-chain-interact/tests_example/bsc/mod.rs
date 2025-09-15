use std::collections::HashMap;

use wallet_chain_interact::eth::{EthChain, Provider};
use wallet_transport::client::RpcClient;
use wallet_utils::init_test_log;
mod base;
mod multisig;

fn get_chain() -> EthChain {
    init_test_log();

    let rpc = "";

    let header = None;
    let client = RpcClient::new(&rpc, header).unwrap();
    let provider = Provider::new(client).unwrap();
    let network = wallet_types::chain::network::NetworkKind::Mainnet;
    let eth = EthChain::new(provider, network).unwrap();
    eth
}
