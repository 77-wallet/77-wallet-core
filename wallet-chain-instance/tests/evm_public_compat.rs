use wallet_chain_instance::instance::{Address, ChainObject, eth::EthereumInstance};
use wallet_types::chain::{address::r#type::AddressType, chain::ChainCode, network::NetworkKind};

// Compile the original public construction and exhaustive match shapes used by consumers.
#[test]
fn evm_public_variants_remain_source_compatible() {
    for chain in [ChainCode::Ethereum, ChainCode::BnbSmartChain] {
        let inner = EthereumInstance {
            chain_code: chain,
            network: NetworkKind::Mainnet,
        };
        let expected = if chain == ChainCode::Ethereum {
            ChainObject::Eth(inner)
        } else {
            ChainObject::Bnb(inner)
        };
        let actual: ChainObject = (&chain, &AddressType::Other, NetworkKind::Mainnet)
            .try_into()
            .unwrap();
        assert_eq!(actual, expected);
        let identity = match actual {
            ChainObject::Eth(i) | ChainObject::Bnb(i) => i.chain_code,
            ChainObject::Trx(_)
            | ChainObject::Sol(_)
            | ChainObject::Btc(_)
            | ChainObject::Ltc(_)
            | ChainObject::Dog(_)
            | ChainObject::Sui(_)
            | ChainObject::Ton(_) => panic!("expected EthereumInstance"),
        };
        assert_eq!(identity, chain);
    }
    let address = alloy::primitives::Address::ZERO;
    assert_eq!(
        Address::BnbAddress(address).to_string(),
        Address::EthAddress(address).to_string()
    );
}
