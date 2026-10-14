pub mod base;
pub mod provider;
use wallet_chain_interact::sui::{Provider, SuiChain};
use wallet_utils::init_test_log;

const KEY: &str = "8d724382-690c-4f13-9293-6ee3e5b95761";

fn header() -> std::collections::HashMap<String, String> {
    std::collections::HashMap::from([("access-key".to_string(), KEY.to_string())])
}

fn get_chain() -> SuiChain {
    init_test_log();

    // let endpoint = "https://sui.grpc.ankr.com:443";
    let endpoint = "http://100.97.77.28:8001";

    let provider = Provider::new(&endpoint, Some(header())).unwrap();
    SuiChain::new(provider).unwrap()
}

fn get_provider() -> Provider {
    init_test_log();
    let endpoint = "http://100.97.77.28:8001";

    Provider::new(&endpoint, Some(header())).unwrap()
}
