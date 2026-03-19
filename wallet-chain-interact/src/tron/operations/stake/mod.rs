pub mod delegate;
pub use delegate::*;

pub mod freeze;
pub use freeze::*;

pub mod undelegate;
pub use undelegate::*;

pub mod unfreeze;
pub use unfreeze::*;

pub mod vote;
pub use vote::*;

#[derive(serde::Serialize, Debug, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    ENERGY,
    BANDWIDTH,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceType::ENERGY => write!(f, "ENERGY"),
            ResourceType::BANDWIDTH => write!(f, "BANDWIDTH"),
        }
    }
}

impl ResourceType {
    pub fn to_i8(&self) -> i8 {
        match self {
            ResourceType::ENERGY => 1,
            ResourceType::BANDWIDTH => 0,
        }
    }
}
impl TryFrom<&str> for ResourceType {
    type Error = crate::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_ref() {
            "energy" => Ok(ResourceType::ENERGY),
            "bandwidth" => Ok(ResourceType::BANDWIDTH),
            _ => Err(crate::Error::InvalidResourceType(value.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceType;

    #[test]
    fn test_invalid_resource_type() {
        assert!(ResourceType::try_from("invalid").is_err());
        assert_eq!(ResourceType::try_from("energy").unwrap(), ResourceType::ENERGY);
        assert_eq!(
            ResourceType::try_from("bandwidth").unwrap(),
            ResourceType::BANDWIDTH
        );
    }
}
