#[derive(Clone, Copy, Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NetworkKind {
    Mainnet,
    Testnet,
    Regtest,
}
impl TryFrom<&str> for NetworkKind {
    type Error = crate::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_ref() {
            "mainnet" => Ok(NetworkKind::Mainnet),
            "testnet" => Ok(NetworkKind::Testnet),
            "regtest" => Ok(NetworkKind::Regtest),
            _ => Err(crate::Error::InvalidNetworkKind(value.to_string())),
        }
    }
}

impl std::str::FromStr for NetworkKind {
    type Err = crate::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value)
    }
}

impl Into<bitcoin::NetworkKind> for NetworkKind {
    fn into(self) -> bitcoin::NetworkKind {
        match self {
            NetworkKind::Mainnet => bitcoin::NetworkKind::Main,
            NetworkKind::Testnet | NetworkKind::Regtest => bitcoin::NetworkKind::Test,
        }
    }
}

impl Into<litecoin::NetworkKind> for NetworkKind {
    fn into(self) -> litecoin::NetworkKind {
        match self {
            NetworkKind::Mainnet => litecoin::NetworkKind::Main,
            NetworkKind::Testnet | NetworkKind::Regtest => litecoin::NetworkKind::Test,
        }
    }
}

impl Into<dogcoin::NetworkKind> for NetworkKind {
    fn into(self) -> dogcoin::NetworkKind {
        match self {
            NetworkKind::Mainnet => dogcoin::NetworkKind::Main,
            NetworkKind::Testnet | NetworkKind::Regtest => dogcoin::NetworkKind::Test,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NetworkKind;

    #[test]
    fn test_parse_network_kind() {
        assert_eq!(NetworkKind::try_from("mainnet").unwrap(), NetworkKind::Mainnet);
        assert_eq!(NetworkKind::try_from("testnet").unwrap(), NetworkKind::Testnet);
        assert_eq!(NetworkKind::try_from("regtest").unwrap(), NetworkKind::Regtest);
        assert!(NetworkKind::try_from("invalid").is_err());
    }
}
