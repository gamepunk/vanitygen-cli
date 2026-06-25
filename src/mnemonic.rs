//! BIP39 mnemonic generation and address derivation.
//!
//! Generates a random 256-bit entropy → BIP39 mnemonic (24 words) →
//! BIP39 seed → BIP32 master key → standard derivation paths →
//! all four address types.
//!
//! This allows users to import the mnemonic phrase into any BIP39/BIP32
//! compatible wallet (e.g. Electrum, Ledger, Trezor, OneKey) and recover
//! the same set of addresses.

use bip39::Mnemonic;
use bitcoin::{
    bip32::{DerivationPath, Xpriv},
    secp256k1::Secp256k1,
    Network,
};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::address::derive_all;
use crate::error::Error;
use crate::wif;

/// Standard BIP44 derivation paths for each address type.
const DERIVATION_PATHS: &[(&str, &str)] = &[
    ("m/44'/0'/0'/0/0", "Legacy (P2PKH)"),
    ("m/49'/0'/0'/0/0", "Nested SegWit (P2SH)"),
    ("m/84'/0'/0'/0/0", "Native SegWit (P2WPKH)"),
    ("m/86'/0'/0'/0/0", "Taproot (P2TR)"),
];

/// Result of a mnemonic generation.
#[derive(Debug, Clone)]
pub struct MnemonicResult {
    /// The BIP39 mnemonic phrase (24 words).
    pub phrase: String,
    /// Derived addresses per standard derivation path.
    pub paths: Vec<PathInfo>,
}

/// Addresses derived from a specific BIP32 path.
#[derive(Debug, Clone)]
pub struct PathInfo {
    pub label: &'static str,
    pub path: String,
    pub wif: String,
    pub legacy: String,
    pub p2sh: String,
    pub segwit: String,
    pub taproot: String,
}

/// Generate a random BIP39 mnemonic (256-bit / 24 words) and derive
/// standard addresses for each BIP44 / BIP49 / BIP84 / BIP86 path.
pub fn generate_random() -> Result<MnemonicResult, Error> {
    // ── 256 bits of true random entropy ────────────────────────────
    let mut entropy = [0u8; 32];
    OsRng.fill_bytes(&mut entropy);

    // ── BIP39 mnemonic ─────────────────────────────────────────────
    let mnemonic = Mnemonic::from_entropy(&entropy)
        .map_err(|e| Error::InvalidWif(format!("BIP39 entropy: {e}")))?;
    let phrase = mnemonic.to_string();

    // ── BIP39 seed → BIP32 master key ──────────────────────────────
    let seed = mnemonic.to_seed("");
    let secp = Secp256k1::new();
    let master = Xpriv::new_master(Network::Bitcoin, &seed).map_err(Error::Bip32)?;

    // ── Derive each standard path ───────────────────────────────────
    let mut paths = Vec::with_capacity(DERIVATION_PATHS.len());
    for &(path_str, label) in DERIVATION_PATHS {
        let path: DerivationPath = path_str.parse().map_err(Error::Bip32)?;
        let child = master.derive_priv(&secp, &path).map_err(Error::Bip32)?;

        let set = derive_all(&secp, &child.private_key, true, Network::Bitcoin)?;
        let wif_str = wif::format_wif(&child.private_key, true, Network::Bitcoin);

        paths.push(PathInfo {
            label,
            path: path_str.to_string(),
            wif: wif_str,
            legacy: set.legacy.to_string(),
            p2sh: set.p2sh_segwit.to_string(),
            segwit: set.native_segwit.to_string(),
            taproot: set.taproot.to_string(),
        });
    }

    Ok(MnemonicResult { phrase, paths })
}
