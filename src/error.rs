//! Error types for the vanity address generator.

use std::fmt;

/// Top-level error enum for the application.
#[derive(Debug)]
pub enum Error {
    /// Invalid or unsupported prefix string.
    InvalidPrefix(String),
    /// Invalid WIF (wrong length, bad checksum, wrong network, etc.).
    InvalidWif(String),
    /// Thread pool / join failure.
    ThreadPool(String),
    /// Wrapper for bitcoin crate errors.
    Bitcoin(bitcoin::address::ParseError),
    /// Wrapper for secp256k1 errors.
    Secp256k1(bitcoin::secp256k1::Error),
    /// BIP32 / BIP39 derivation error.
    Bip32(bitcoin::bip32::Error),
    /// Wrapper for I/O errors.
    Io(std::io::Error),
    /// General error with a message.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidPrefix(msg) => write!(f, "invalid prefix: {msg}"),
            Error::InvalidWif(msg) => write!(f, "invalid WIF: {msg}"),
            Error::ThreadPool(msg) => write!(f, "thread pool error: {msg}"),
            Error::Bitcoin(e) => write!(f, "bitcoin: {e}"),
            Error::Secp256k1(e) => write!(f, "secp256k1: {e}"),
            Error::Bip32(e) => write!(f, "BIP32: {e}"),
            Error::Io(e) => write!(f, "I/O: {e}"),
            Error::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Bitcoin(e) => Some(e),
            Error::Secp256k1(e) => Some(e),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<bitcoin::address::ParseError> for Error {
    fn from(e: bitcoin::address::ParseError) -> Self {
        Error::Bitcoin(e)
    }
}

impl From<bitcoin::secp256k1::Error> for Error {
    fn from(e: bitcoin::secp256k1::Error) -> Self {
        Error::Secp256k1(e)
    }
}

impl From<bitcoin::bip32::Error> for Error {
    fn from(e: bitcoin::bip32::Error) -> Self {
        Error::Bip32(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
