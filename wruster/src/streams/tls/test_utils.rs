use std::{io, path::PathBuf};

use super::{Certificate, PrivateKey};

pub fn load_test_certificate() -> io::Result<Certificate> {
    let mut cert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    cert_path.push("tests/certs/cert.pem");
    Certificate::read_from(cert_path.to_str().unwrap())
}

pub fn load_test_private_key() -> Result<PrivateKey, io::Error> {
    let mut key_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    key_path.push("tests/certs/key.pem");
    PrivateKey::read_from(key_path.to_str().unwrap())
}
