use std::fs;
use std::io::BufReader;

pub fn load_certs(filename: &str) -> Result<Vec<rustls::Certificate>, LoadCertificateError> {
    let cert_file = fs::File::open(filename)?;
    let mut reader = BufReader::new(cert_file);
    let certs = rustls::internal::pemfile::certs(&mut reader)?;
    Ok(certs)
}

pub fn load_private_key(filename: &str) -> Result<rustls::PrivateKey, LoadPrivateKeyError> {
    let rsa_keys = {
        let key_file = fs::File::open(filename)?;
        let mut reader = BufReader::new(key_file);
        rustls::internal::pemfile::rsa_private_keys(&mut reader)?
    };

    let pkcs8_keys = {
        let keyfile = fs::File::open(filename)?;
        let mut reader = BufReader::new(keyfile);
        rustls::internal::pemfile::pkcs8_private_keys(&mut reader)?
    };

    // prefer to load pkcs8 keys
    if !pkcs8_keys.is_empty() {
        Ok(pkcs8_keys[0].clone())
    } else {
        if rsa_keys.is_empty() {
            return Err(LoadPrivateKeyError::RsaKeyIsEmpty);
        }
        Ok(rsa_keys[0].clone())
    }
}

#[derive(Debug)]
pub enum LoadCertificateError {
    CannotOpenFile(std::io::Error),
    CannotExtractSertificates,
}

impl From<std::io::Error> for LoadCertificateError {
    fn from(err: std::io::Error) -> Self {
        LoadCertificateError::CannotOpenFile(err)
    }
}

impl From<()> for LoadCertificateError {
    fn from(_err: ()) -> Self {
        LoadCertificateError::CannotExtractSertificates
    }
}

impl std::fmt::Display for LoadCertificateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for LoadCertificateError {}

#[derive(Debug)]
pub enum LoadPrivateKeyError {
    CannotOpenFile(std::io::Error),
    RsaPrivateKeys,
    RsaKeyIsEmpty,
}

impl From<std::io::Error> for LoadPrivateKeyError {
    fn from(err: std::io::Error) -> Self {
        LoadPrivateKeyError::CannotOpenFile(err)
    }
}

impl From<()> for LoadPrivateKeyError {
    fn from(_err: ()) -> Self {
        LoadPrivateKeyError::RsaPrivateKeys
    }
}

impl std::fmt::Display for LoadPrivateKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for LoadPrivateKeyError {}
