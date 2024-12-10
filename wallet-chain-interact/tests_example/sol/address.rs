use coins_bip39::{English, Mnemonic};
use solana_sdk::{
    derivation_path::DerivationPath, signature::keypair_from_seed_and_derivation_path,
    signer::Signer,
};
fn seed() -> [u8; 64] {
    let mnemonic = "victory member rely dirt treat woman boring tomato two hollow erosion drop";
    let mnemonic = Mnemonic::<English>::new_from_phrase(mnemonic).unwrap();

    // 生成种子
    let seed = mnemonic.to_seed(Some("")).unwrap();
    seed
}

#[test]
fn test_addr1() {
    let path = "m/44'/501'/0'/0'";
    let path = DerivationPath::from_absolute_path_str(&path).unwrap();
    let keypair = keypair_from_seed_and_derivation_path(&seed(), Some(path)).unwrap();

    let pubkey = keypair.pubkey();
    println!("address: {}", pubkey);

    assert_eq!(
        "GE93MHXVvnsbhxu7Ttpp7zTiipJeCX3QFXueSK2dCJe6",
        pubkey.to_string()
    )
}

#[test]
fn test_addr2() {
    let path = "m/44'/501'/1'/0'";
    let path = DerivationPath::from_absolute_path_str(&path).unwrap();
    let keypair = keypair_from_seed_and_derivation_path(&seed(), Some(path)).unwrap();

    let pubkey = keypair.pubkey();
    println!("address: {}", pubkey);

    assert_eq!(
        "MmqgDWhS59oXWVuVtogpvj6k5RLny2ZHCGwDQX1yqkC",
        pubkey.to_string()
    )
}

#[test]
fn test_addr3() {
    let path = "m/44'/501'/2'/0'";
    let path = DerivationPath::from_absolute_path_str(&path).unwrap();
    let keypair = keypair_from_seed_and_derivation_path(&seed(), Some(path)).unwrap();

    let pubkey = keypair.pubkey();
    println!("address: {}", pubkey);

    assert_eq!(
        "Ey3PmUxYJXK6DrtNSq47aE86tcMf9u6EbM89Dh76etPt",
        pubkey.to_string()
    )
}

#[test]
fn test_key() {
    let key1 = "13924d06e80f229a75a4f7b4e434b47b6532c4f15e2ffd68ebc2b79db05bb237";
    let key1 = solana_sdk::bs58::encode(key1).into_string();

    assert_eq!(
        key1,
        "PhKgs4sb76HtfzZv2N5ZxFfupPtKQghRdE3c8q2UW65JokknVwtPvsGnzQYtURAtf6Z5u1DFVtNxqzwkMJJ7VwQ",
    );
}
