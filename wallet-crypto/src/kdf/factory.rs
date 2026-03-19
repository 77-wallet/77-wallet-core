use serde::{Deserialize, Serialize};

use crate::{
    crypto::encrypted_json::encrypted::EncryptedJson,
    kdf::{
        KdfParams, KeyDerivationFunction,
        argon2id::Argon2idKdf,
        pbkdf2::{Pbkdf2Kdf, Pbkdf2Params},
        scrypt_::{ScryptKdf, ScryptParams},
    },
    utils::HexBytes,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Types of key derivition functions supported by the Web3 Secret Storage.
pub enum KdfAlgorithm {
    Pbkdf2,
    Scrypt,
    Argon2id,
}

pub struct KdfFactory;

impl KdfFactory {
    pub fn create(
        algorithm: &KdfAlgorithm,
        salt: &[u8],
    ) -> Result<Box<dyn KeyDerivationFunction>, crate::Error> {
        match algorithm {
            KdfAlgorithm::Scrypt => {
                let params = ScryptParams::default().with_salt(salt);
                Ok(Box::new(ScryptKdf::new(params)))
            }
            KdfAlgorithm::Pbkdf2 => {
                let params = Pbkdf2Params {
                    c: 262_144,
                    dklen: 32,
                    prf: "hmac-sha256".to_string(),
                    salt: HexBytes(salt.to_vec()),
                };
                Ok(Box::new(Pbkdf2Kdf::new(params)))
            }
            KdfAlgorithm::Argon2id => {
                let params = Argon2idKdf::recommended_params_with_salt(salt);
                Ok(Box::new(params))
            }
        }
    }

    pub fn create_from_encrypted_data(
        keystore: &EncryptedJson,
    ) -> Result<Box<dyn KeyDerivationFunction>, crate::Error> {
        let kdf: Box<dyn KeyDerivationFunction> = match &keystore.crypto.kdfparams {
            KdfParams::Pbkdf2(p) => Box::new(Pbkdf2Kdf::new(p.to_owned())),
            KdfParams::Scrypt(p) => Box::new(ScryptKdf::new(p.to_owned())),
            KdfParams::Argon2id(p) => Box::new(Argon2idKdf::new(p.to_owned())),
        };

        Ok(kdf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kdf_factory_pbkdf2() {
        let salt = b"0123456789abcdef";
        let kdf = KdfFactory::create(&KdfAlgorithm::Pbkdf2, salt).unwrap();

        assert_eq!(kdf.algorithm(), KdfAlgorithm::Pbkdf2);
        assert_eq!(kdf.params(), KdfParams::Pbkdf2(Pbkdf2Params {
            c: 262_144,
            dklen: 32,
            prf: "hmac-sha256".to_string(),
            salt: HexBytes(salt.to_vec()),
        }));

        let derived = kdf.derive_key(b"password").unwrap();
        assert_eq!(derived.len(), 32);
    }
}
