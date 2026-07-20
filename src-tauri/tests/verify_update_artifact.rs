use std::{env, fs};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};

const ARCHIVE_ENV: &str = "LEVELOG_VERIFY_UPDATE_ARCHIVE";
const SIGNATURE_ENV: &str = "LEVELOG_VERIFY_UPDATE_SIGNATURE";
const PUBLIC_KEY_ENV: &str = "LEVELOG_VERIFY_UPDATE_PUBLIC_KEY";

fn required_env(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("required verifier input {name} is not set"))
}

fn decode_base64_text(encoded: &str, kind: &str) -> String {
    let decoded = STANDARD
        .decode(encoded.trim())
        .unwrap_or_else(|_| panic!("{kind} is not valid base64"));
    String::from_utf8(decoded).unwrap_or_else(|_| panic!("decoded {kind} is not UTF-8"))
}

#[test]
#[ignore = "release-only verifier; requires archive, signature, and public-key environment inputs"]
fn release_update_archive_matches_configured_public_key() {
    let archive_path = required_env(ARCHIVE_ENV);
    let signature_path = required_env(SIGNATURE_ENV);
    let encoded_public_key = required_env(PUBLIC_KEY_ENV);

    let public_key_text = decode_base64_text(&encoded_public_key, "updater public key");
    let public_key = PublicKey::decode(&public_key_text)
        .expect("decoded updater public key is not a valid Minisign public key");

    let encoded_signature =
        fs::read_to_string(&signature_path).expect("could not read updater signature file");
    let signature_text = decode_base64_text(&encoded_signature, "updater signature");
    let signature = Signature::decode(&signature_text)
        .expect("decoded updater signature is not a valid Minisign signature");

    let archive = fs::read(&archive_path).expect("could not read updater archive");
    public_key
        .verify(&archive, &signature, true)
        .expect("updater archive signature does not match the configured public key");
}
