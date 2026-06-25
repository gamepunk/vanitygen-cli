//! Address derivation for all four Bitcoin address types.
//!
//! All hash / curve operations are delegated to `rust-bitcoin` / `secp256k1`.
//! This module only contains glue logic.

use crate::error::Error;
use bech32::{hrp, segwit};
use bitcoin::{
    secp256k1::{PublicKey as SecpPublicKey, Secp256k1, SecretKey},
    Address, CompressedPublicKey, Network, PublicKey, ScriptBuf,
};

/// A set of all four address types derived from a single key pair.
#[derive(Debug, Clone)]
pub struct AddressSet {
    pub legacy: Address,
    pub p2sh_segwit: Address,
    pub native_segwit: Address,
    pub taproot: Address,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Derive all four address types from a [`SecretKey`].
///
/// * `secp` – secp256k1 context (needs `Signing` + `Verification` capabilities).
/// * `sk`   – the private key.
/// * `compressed` – whether to use the compressed public key format
///   (affects Legacy addresses; P2SH, SegWit and Taproot always use
///   compressed / x-only keys internally).
/// * `network` – `Bitcoin`, `Testnet`, `Signet`, or `Regtest`.
pub fn derive_all<C: bitcoin::secp256k1::Signing + bitcoin::secp256k1::Verification>(
    secp: &Secp256k1<C>,
    sk: &SecretKey,
    compressed: bool,
    network: Network,
) -> Result<AddressSet, Error> {
    let secp_pk = SecpPublicKey::from_secret_key(secp, sk);
    let pubkey = PublicKey {
        inner: secp_pk,
        compressed,
    };
    // P2SH, SegWit and Taproot always require the compressed public key
    // internally, regardless of the `compressed` flag (which only affects
    // Legacy P2PKH).
    let pubkey_compressed = PublicKey {
        inner: secp_pk,
        compressed: true,
    };

    Ok(AddressSet {
        legacy: Address::p2pkh(pubkey, network),
        p2sh_segwit: p2sh_wpkh(&pubkey_compressed, network),
        native_segwit: native_segwit(&pubkey_compressed, network),
        taproot: derive_taproot(secp, &secp_pk, network)?,
    })
}

/// Derive only the requested address type (used during hot search loop).
pub fn derive_single<C: bitcoin::secp256k1::Signing + bitcoin::secp256k1::Verification>(
    secp: &Secp256k1<C>,
    sk: &SecretKey,
    compressed: bool,
    network: Network,
    addr_type: crate::cli::AddressType,
) -> Result<Address, Error> {
    use crate::cli::AddressType::*;
    let secp_pk = SecpPublicKey::from_secret_key(secp, sk);
    // P2SH / SegWit / Taproot always need the compressed public key.
    let pubkey = PublicKey {
        inner: secp_pk,
        compressed: true,
    };

    match addr_type {
        Legacy => Ok(Address::p2pkh(
            PublicKey {
                inner: secp_pk,
                compressed,
            },
            network,
        )),
        P2sh => Ok(p2sh_wpkh(&pubkey, network)),
        Segwit => Ok(native_segwit(&pubkey, network)),
        Taproot => derive_taproot(secp, &secp_pk, network),
    }
}

// ---------------------------------------------------------------------------
// Fast hot-path helpers (avoid Address type overhead in search loop)
// ---------------------------------------------------------------------------

/// SHA256 then RIPEMD160 (used in both P2PKH and P2SH).
#[inline]
pub fn hash160(data: &[u8]) -> [u8; 20] {
    use ripemd::Ripemd160;
    use sha2::{Digest, Sha256};
    let mut out = [0u8; 20];
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    out.copy_from_slice(&ripe);
    out
}

/// Double-SHA256, returns the full 32-byte digest.
#[inline]
pub fn double_sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut out = [0u8; 32];
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    out.copy_from_slice(&second);
    out
}

/// Fast Base58Check encode for a payload (version byte + data).
/// Returns just the address string – no `Address` object created.
#[inline]
pub fn base58check_encode_fast(payload: &[u8]) -> String {
    let checksum = double_sha256(payload);
    let mut full = Vec::with_capacity(payload.len() + 4);
    full.extend_from_slice(payload);
    full.extend_from_slice(&checksum[..4]);
    bs58::encode(full).into_string()
}

/// Fast P2PKH (Legacy) address string from serialized public key bytes.
/// Skips the `Address` type entirely – returns the Base58 string directly.
#[inline]
pub fn p2pkh_address_fast(pubkey_bytes: &[u8]) -> String {
    let h160 = hash160(pubkey_bytes);
    let mut payload = [0u8; 21];
    payload[0] = 0x00;
    payload[1..21].copy_from_slice(&h160);
    base58check_encode_fast(&payload)
}

/// Fast P2SH-P2WPKH address string from a serialized compressed public key.
#[inline]
pub fn p2sh_wpkh_address_fast(pubkey_bytes: &[u8]) -> String {
    let pk_hash = hash160(pubkey_bytes);
    // Witness program: OP_0 (0x00) + 0x14 + pubkey_hash
    let witness_script = [&[0x00u8, 0x14], &pk_hash[..]].concat();
    let script_hash = hash160(&witness_script);
    let mut payload = [0u8; 21];
    payload[0] = 0x05;
    payload[1..21].copy_from_slice(&script_hash);
    base58check_encode_fast(&payload)
}

/// Fast native SegWit (P2WPKH) address string from a compressed public key.
/// Skips the `Address` type entirely – returns the Bech32 string directly.
#[inline]
pub fn native_segwit_address_fast(pubkey_bytes: &[u8]) -> String {
    let h160 = hash160(pubkey_bytes);
    segwit::encode(hrp::BC, segwit::VERSION_0, &h160)
        .expect("valid P2WPKH address (20-byte witness program)")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Derive a P2TR (Taproot) address, key-path spend only (no script tree).
///
/// The `rust-bitcoin` crate internally computes the tweak
///   `t = H(P || C)`  (with `C = None` for key-spend-only)
/// and returns `Q = P + t·G`.  The address encodes the x-coordinate of `Q`.
fn derive_taproot<C: bitcoin::secp256k1::Verification>(
    secp: &Secp256k1<C>,
    secp_pk: &SecpPublicKey,
    network: Network,
) -> Result<Address, Error> {
    let (x_only, _parity) = secp_pk.x_only_public_key();
    // `None` merkle_root → key-path-only spend.
    Ok(Address::p2tr(secp, x_only, None, network))
}

/// Build a P2SH address wrapping a P2WPKH (SegWit v0) witness program.
///
/// In `bitcoin` v0.32 there is no dedicated `p2sh_from_wpkh`; we manually
/// create the P2WPKH script and then wrap it via `Address::p2sh`.
fn p2sh_wpkh(pubkey: &PublicKey, network: Network) -> Address {
    let wpkh_script = ScriptBuf::new_p2wpkh(
        &pubkey
            .wpubkey_hash()
            .expect("P2SH-P2WPKH requires a compressed public key"),
    );
    Address::p2sh(&wpkh_script, network).expect("valid P2SH script")
}

/// Build a native SegWit (P2WPKH) address from a [`PublicKey`].
/// Requires a compressed public key.
fn native_segwit(pubkey: &PublicKey, network: Network) -> Address {
    let compressed =
        CompressedPublicKey::try_from(*pubkey).expect("P2WPKH requires a compressed public key");
    Address::p2wpkh(&compressed, network)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::Network;

    /// Known test vector: secret key = 1 (compressed).
    #[test]
    fn test_key_1_compressed() {
        let secp = Secp256k1::new();
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        let sk = SecretKey::from_slice(&bytes).unwrap();

        let set = derive_all(&secp, &sk, true, Network::Bitcoin).unwrap();

        assert_eq!(set.legacy.to_string(), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
        assert_eq!(
            set.p2sh_segwit.to_string(),
            "3JvL6Ymt8MVWiCNHC7oWU6nLeHNJKLZGLN"
        );
        assert_eq!(
            set.native_segwit.to_string(),
            "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4"
        );
        assert_eq!(
            set.taproot.to_string(),
            "bc1pmfr3p9j00pfxjh0zmgp99y8zftmd3s5pmedqhyptwy6lm87hf5sspknck9"
        );
    }

    /// Known test vector: secret key = 1 (uncompressed).
    #[test]
    fn test_key_1_uncompressed() {
        let secp = Secp256k1::new();
        let mut bytes = [0u8; 32];
        bytes[31] = 1;
        let sk = SecretKey::from_slice(&bytes).unwrap();

        let set = derive_all(&secp, &sk, false, Network::Bitcoin).unwrap();

        assert_eq!(set.legacy.to_string(), "1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm");
    }

    /// Fast-path helpers match the safe Address API.
    #[test]
    fn test_fast_path_matches_address_api() {
        let secp = Secp256k1::new();
        let mut bytes = [0u8; 32];
        bytes[31] = 42; // arbitrary non-1 key
        let sk = SecretKey::from_slice(&bytes).unwrap();
        let secp_pk = SecpPublicKey::from_secret_key(&secp, &sk);
        let pk_compressed = secp_pk.serialize();
        let pk_uncompressed = secp_pk.serialize_uncompressed();

        // Legacy compressed
        let fast = p2pkh_address_fast(&pk_compressed);
        let safe = Address::p2pkh(
            &PublicKey {
                inner: secp_pk,
                compressed: true,
            },
            Network::Bitcoin,
        )
        .to_string();
        assert_eq!(fast, safe, "fast P2PKH compressed mismatch");

        // Legacy uncompressed
        let fast_u = p2pkh_address_fast(&pk_uncompressed);
        let safe_u = Address::p2pkh(
            &PublicKey {
                inner: secp_pk,
                compressed: false,
            },
            Network::Bitcoin,
        )
        .to_string();
        assert_eq!(fast_u, safe_u, "fast P2PKH uncompressed mismatch");

        // P2SH-P2WPKH (always compressed)
        let fast_p2sh = p2sh_wpkh_address_fast(&pk_compressed);
        let pubkey_c = PublicKey {
            inner: secp_pk,
            compressed: true,
        };
        let wpkh_script = ScriptBuf::new_p2wpkh(&pubkey_c.wpubkey_hash().unwrap());
        let safe_p2sh = Address::p2sh(&wpkh_script, Network::Bitcoin)
            .unwrap()
            .to_string();
        assert_eq!(fast_p2sh, safe_p2sh, "fast P2SH mismatch");

        // Native SegWit (P2WPKH) – always compressed
        let fast_segwit = native_segwit_address_fast(&pk_compressed);
        let compressed_pk = CompressedPublicKey::try_from(pubkey_c).unwrap();
        let safe_segwit = Address::p2wpkh(&compressed_pk, Network::Bitcoin).to_string();
        assert_eq!(fast_segwit, safe_segwit, "fast SegWit mismatch");
    }

    /// Test hash160 with known vectors.
    #[test]
    fn test_hash160_empty() {
        let h = hash160(b"");
        assert_eq!(
            h,
            [
                0xb4, 0x72, 0xa2, 0x66, 0xd0, 0xbd, 0x89, 0xc1, 0x37, 0x06, 0xa4, 0x13, 0x2c, 0xcf,
                0xb1, 0x6f, 0x7c, 0x3b, 0x9f, 0xcb
            ],
            "hash160('') mismatch"
        );
    }

    /// Test double_sha256 with known vector.
    #[test]
    fn test_double_sha256_hello() {
        let d = double_sha256(b"hello");
        assert_eq!(
            d,
            [
                0x95, 0x95, 0xc9, 0xdf, 0x90, 0x07, 0x51, 0x48, 0xeb, 0x06, 0x86, 0x03, 0x65, 0xdf,
                0x33, 0x58, 0x4b, 0x75, 0xbf, 0xf7, 0x82, 0xa5, 0x10, 0xc6, 0xcd, 0x48, 0x83, 0xa4,
                0x19, 0x83, 0x3d, 0x50
            ],
            "double_sha256('hello') mismatch"
        );
    }

    /// Key = 0 is invalid (curve order check).
    #[test]
    fn test_zero_key_rejected() {
        let bytes = [0u8; 32]; // all zeros = invalid key
        assert!(SecretKey::from_slice(&bytes).is_err());
    }
}
