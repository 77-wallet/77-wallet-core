pub mod derivation_path;
pub mod error;
pub mod instance;

pub use error::{Error, keypair::KeypairError};
pub use instance::btc::address::generate_address_with_xpriv;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub fn add_index(path: &str, index: u32, hard: bool) -> Result<String, Error> {
    let parts: Vec<&str> = path.rsplitn(2, '/').collect();
    if parts.len() != 2 {
        return Err(Error::HdPath("Invalid derivation path".to_string()));
    }
    let index = if hard {
        format!("{index}'")
    } else {
        format!("{index}")
    };
    Ok(format!("{}/{}", parts[1], index))
}

pub fn add_solana_index(path: &str, index: u32) -> Result<String, Error> {
    let parts: Vec<&str> = path.splitn(4, '\'').collect();
    if parts.len() != 4 {
        return Err(Error::HdPath("Invalid derivation path".to_string()));
    }

    Ok(format!("{}'{}'/{}'{}", parts[0], parts[1], index, parts[3]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn test_add_index_invalid_path() {
        assert!(add_index("invalid", 0, false).is_err());
        assert!(add_solana_index("invalid", 0).is_err());
    }
}
