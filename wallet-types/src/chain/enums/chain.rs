#[derive(Debug, PartialEq, Clone, serde::Serialize, Copy)]
pub enum ChainCode {
    Tron,
    Bitcoin,
    Solana,
    Ethereum,
    BnbSmartChain,
    Polygon,
    Avalanche,
    Fantom,
    Arbitrum,
    Base,
    Scroll,
    Litecoin,
    Dogcoin,
    Sui,
    Ton,
}

impl TryFrom<&str> for ChainCode {
    type Error = crate::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let res = match value {
            crate::constant::chain_code::TRON => ChainCode::Tron,
            crate::constant::chain_code::BTC => ChainCode::Bitcoin,
            crate::constant::chain_code::SOLANA => ChainCode::Solana,
            crate::constant::chain_code::ETHEREUM => ChainCode::Ethereum,
            crate::constant::chain_code::BNB => ChainCode::BnbSmartChain,
            crate::constant::chain_code::POLYGON => ChainCode::Polygon,
            crate::constant::chain_code::AVALANCHE => ChainCode::Avalanche,
            crate::constant::chain_code::FANTOM => ChainCode::Fantom,
            crate::constant::chain_code::ARBITRUM => ChainCode::Arbitrum,
            crate::constant::chain_code::BASE => ChainCode::Base,
            crate::constant::chain_code::SCROLL => ChainCode::Scroll,
            crate::constant::chain_code::LTC => ChainCode::Litecoin,
            crate::constant::chain_code::DOG => ChainCode::Dogcoin,
            crate::constant::chain_code::TON => ChainCode::Ton,
            crate::constant::chain_code::SUI => ChainCode::Sui,
            _ => return Err(crate::Error::UnknownChainCode),
        };
        Ok(res)
    }
}

impl std::fmt::Display for ChainCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainCode::Ethereum => write!(f, "{}", crate::constant::chain_code::ETHEREUM),
            ChainCode::Tron => write!(f, "{}", crate::constant::chain_code::TRON),
            ChainCode::Solana => write!(f, "{}", crate::constant::chain_code::SOLANA),
            ChainCode::BnbSmartChain => write!(f, "{}", crate::constant::chain_code::BNB),
            ChainCode::Polygon => write!(f, "{}", crate::constant::chain_code::POLYGON),
            ChainCode::Avalanche => write!(f, "{}", crate::constant::chain_code::AVALANCHE),
            ChainCode::Fantom => write!(f, "{}", crate::constant::chain_code::FANTOM),
            ChainCode::Arbitrum => write!(f, "{}", crate::constant::chain_code::ARBITRUM),
            ChainCode::Base => write!(f, "{}", crate::constant::chain_code::BASE),
            ChainCode::Scroll => write!(f, "{}", crate::constant::chain_code::SCROLL),
            ChainCode::Bitcoin => write!(f, "{}", crate::constant::chain_code::BTC),
            ChainCode::Litecoin => write!(f, "{}", crate::constant::chain_code::LTC),
            ChainCode::Dogcoin => write!(f, "{}", crate::constant::chain_code::DOG),
            ChainCode::Sui => write!(f, "{}", crate::constant::chain_code::SUI),
            ChainCode::Ton => write!(f, "{}", crate::constant::chain_code::TON),
        }
    }
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct ChainCodes(pub Vec<ChainCode>);

impl TryFrom<u32> for ChainCodes {
    type Error = crate::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let chain_code = match value {
            crate::constant::chain_type::ETH_TYPE => {
                vec![
                    ChainCode::Ethereum,
                    ChainCode::BnbSmartChain,
                    ChainCode::Polygon,
                    ChainCode::Avalanche,
                    ChainCode::Fantom,
                    ChainCode::Arbitrum,
                    ChainCode::Base,
                    ChainCode::Scroll,
                ]
            }
            crate::constant::chain_type::TRON_TYPE => vec![ChainCode::Tron],
            crate::constant::chain_type::SOLANA_TYPE => vec![ChainCode::Solana],
            crate::constant::chain_type::BTC_TYPE => vec![ChainCode::Bitcoin],
            crate::constant::chain_type::LTC_TYPE => vec![ChainCode::Litecoin],
            crate::constant::chain_type::DOG_TYPE => vec![ChainCode::Dogcoin],
            crate::constant::chain_type::SUI_TYPE => vec![ChainCode::Sui],
            _ => return Err(crate::Error::UnknownChainCode),
        };

        Ok(ChainCodes(chain_code))
    }
}

#[cfg(test)]
mod tests {
    use super::{ChainCode, ChainCodes};
    use crate::constant::chain_type::ETH_TYPE;
    use std::collections::HashSet;

    #[test]
    fn chain_code_canonical_evm_codes_round_trip() {
        let cases = [
            ("eth", ChainCode::Ethereum),
            ("bnb", ChainCode::BnbSmartChain),
            ("polygon", ChainCode::Polygon),
            ("avalanche", ChainCode::Avalanche),
            ("fantom", ChainCode::Fantom),
            ("arbitrum", ChainCode::Arbitrum),
            ("base", ChainCode::Base),
            ("scroll", ChainCode::Scroll),
        ];

        for (code, chain) in cases {
            assert_eq!(ChainCode::try_from(code).unwrap(), chain);
            assert_eq!(chain.to_string(), code);
        }
    }

    #[test]
    fn chain_code_rejects_aliases_unknown_and_wrong_case() {
        for code in ["matic", "avax", "ftm", "arb", "Polygon", "unknown"] {
            assert!(ChainCode::try_from(code).is_err());
        }
    }

    #[test]
    fn chain_codes_eth_type_contains_all_unique_evm_chains() {
        let chains = ChainCodes::try_from(ETH_TYPE).unwrap().0;
        let unique: HashSet<_> = chains.iter().map(ToString::to_string).collect();

        assert_eq!(chains.len(), 8);
        assert_eq!(unique.len(), chains.len());
        for expected in [
            ChainCode::Ethereum,
            ChainCode::BnbSmartChain,
            ChainCode::Polygon,
            ChainCode::Avalanche,
            ChainCode::Fantom,
            ChainCode::Arbitrum,
            ChainCode::Base,
            ChainCode::Scroll,
        ] {
            assert!(chains.contains(&expected));
        }
    }

    #[test]
    fn chain_code_non_evm_behavior_is_unchanged() {
        for (code, expected) in [
            ("tron", ChainCode::Tron),
            ("sol", ChainCode::Solana),
            ("btc", ChainCode::Bitcoin),
            ("ltc", ChainCode::Litecoin),
            ("doge", ChainCode::Dogcoin),
            ("sui", ChainCode::Sui),
            ("ton", ChainCode::Ton),
        ] {
            assert_eq!(ChainCode::try_from(code).unwrap(), expected);
            assert_eq!(expected.to_string(), code);
        }
    }
}
