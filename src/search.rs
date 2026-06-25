//! Multi-threaded vanity address search.
//!
//! Each worker thread starts from a random secret key and then walks forward
//! by adding `1` to the secret key (and `G` to the public key) per iteration.
//! This avoids a full scalar multiplication on every step.
//!
//! After `BATCH_SIZE` iterations a thread re-randomises its starting point to
//! avoid approaching the curve-order boundary.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bip39::Mnemonic;
use bitcoin::{
    bip32::{DerivationPath, Xpriv},
    secp256k1::{Scalar, Secp256k1, SecretKey},
    Network,
};
use rand::rngs::OsRng;
use rand::RngCore;
use regex::Regex;

use crate::address::{self as addr, derive_single};
use crate::checkpoint;
use crate::cli::{AddressType, MatchMode};
use crate::error::Error;
use crate::style;
use crate::wif;

/// How many iterations a worker does before re-randomising its starting point.
const BATCH_SIZE: u64 = 5_000_000;

/// Max address indices to scan per BIP32 account before generating a new
/// mnemonic (2³¹ normal child keys exists per account, but that's huge).
/// Result produced when a matching address is found.
#[derive(Debug, Clone)]
pub struct FoundResult {
    pub address: String,
    pub wif: String,
    pub total_attempts: u64,
    /// BIP39 mnemonic phrase, present only when `--mnemonic` was used.
    pub mnemonic_phrase: Option<String>,
    /// BIP32 derivation path, present only when `--mnemonic` was used.
    pub derivation_path: Option<String>,
}

// ── Address-type prefixes ──────────────────────────────────────────────

fn addr_type_prefix(addr_type: AddressType) -> &'static str {
    match addr_type {
        AddressType::Legacy => "1",
        AddressType::P2sh => "3",
        AddressType::Segwit => "bc1q",
        AddressType::Taproot => "bc1p",
    }
}

/// Strip the address-type prefix from `addr` if it matches.
fn strip_addr_prefix(addr: &str, addr_type: AddressType) -> &str {
    let pfx = addr_type_prefix(addr_type);
    if let Some(rest) = addr.strip_prefix(pfx) {
        rest
    } else {
        addr
    }
}

// ── Matching helper ─────────────────────────────────────────────────────

/// Check whether `addr` matches `pattern` according to `mode`.
/// For Regex mode, `re` is a pre-compiled `Regex`; pass `None` otherwise.
/// When `strip_prefix` is true, the address-type prefix is removed before
/// matching, so users can search for "Bit" instead of "1Bit".
pub fn is_match(
    addr: &str,
    pattern: &str,
    mode: MatchMode,
    case_insensitive: bool,
    re: Option<&Regex>,
    addr_type: Option<AddressType>,
    strip_prefix: bool,
) -> bool {
    let match_str = if strip_prefix {
        if let Some(at) = addr_type {
            strip_addr_prefix(addr, at)
        } else {
            addr
        }
    } else {
        addr
    };
    let s = if case_insensitive {
        match_str.to_lowercase()
    } else {
        match_str.to_string()
    };
    match mode {
        MatchMode::Prefix => s.starts_with(pattern),
        MatchMode::Suffix => s.ends_with(pattern),
        MatchMode::Anywhere => s.contains(pattern),
        MatchMode::Regex => {
            if let Some(r) = re {
                r.is_match(addr)
            } else {
                false
            }
        }
    }
}

// -----------------------------------------------------------------------
// Public dispatcher
// -----------------------------------------------------------------------

/// All parameters that control a vanity search.
///
/// Separate from `commands::SearchConfig` because this lives in the
/// search module and doesn't need output/bark fields.
#[derive(Debug, Clone)]
pub struct SearchParams<'a> {
    pub pattern: &'a str,
    pub addr_type: AddressType,
    pub case_insensitive: bool,
    pub compressed: bool,
    pub network: Network,
    pub num_threads: usize,
    pub use_bip32: bool,
    pub quiet: bool,
    pub match_mode: MatchMode,
    pub bip39_words: usize,
    pub target_count: usize,
    pub strip_prefix: bool,
}

/// Launch multiple workers to search for addresses matching the given
/// parameters.  Blocks until `params.target_count` matches are found.
pub fn search(params: SearchParams) -> Result<(Vec<FoundResult>, std::time::Duration), Error> {
    let p = Arc::new(params.pattern.to_string());
    let match_mode = Arc::new(params.match_mode);
    let found_count = Arc::new(AtomicU64::new(0));
    let counter = Arc::new(AtomicU64::new(0));
    let results: Arc<Mutex<Vec<FoundResult>>> = Arc::new(Mutex::new(Vec::new()));

    // Pre-compile regex if needed.
    let re: Option<Regex> = match *match_mode {
        MatchMode::Regex => {
            Some(Regex::new(&p).map_err(|e| Error::Other(format!("Invalid regex pattern: {e}")))?)
        }
        _ => None,
    };
    let re = Arc::new(re);

    // Pre-compute the comparison string for non-regex modes.
    let cmp_pat = if params.case_insensitive {
        p.to_lowercase()
    } else {
        p.to_string()
    };
    let cmp_pat = Arc::new(cmp_pat);

    let start = Instant::now();

    // ── Spawn workers ───────────────────────────────────────────────
    let mut handles = Vec::with_capacity(params.num_threads);
    for _ in 0..params.num_threads {
        let pattern = Arc::clone(&p);
        let cmp_pat = Arc::clone(&cmp_pat);
        let re = Arc::clone(&re);
        let match_mode = Arc::clone(&match_mode);
        let found_count = Arc::clone(&found_count);
        let counter = Arc::clone(&counter);
        let results = Arc::clone(&results);

        if params.use_bip32 {
            handles.push(std::thread::spawn(move || {
                worker_bip32(
                    &pattern,
                    &cmp_pat,
                    &re,
                    match_mode,
                    params.addr_type,
                    params.case_insensitive,
                    params.network,
                    found_count,
                    counter,
                    results,
                    params.bip39_words,
                    params.target_count,
                    params.strip_prefix,
                );
            }));
        } else {
            handles.push(std::thread::spawn(move || {
                worker(
                    &pattern,
                    &cmp_pat,
                    &re,
                    match_mode,
                    params.addr_type,
                    params.case_insensitive,
                    params.compressed,
                    params.network,
                    found_count,
                    counter,
                    results,
                    params.target_count,
                    params.strip_prefix,
                );
            }));
        }
    }

    // ── Progress reporter (main thread) ─────────────────────────────
    let checkpoint_params = checkpoint::SearchParams {
        prefix: p.to_string(),
        address_type: params.addr_type,
        case_insensitive: params.case_insensitive,
        threads: params.num_threads,
    };
    let mut checkpoint_tick: u32 = 0;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(1500));

        let n_found = found_count.load(Ordering::Relaxed) as usize;
        if n_found >= params.target_count {
            break;
        }

        let elapsed = start.elapsed();
        let n = counter.load(Ordering::Relaxed);
        let rate = n as f64 / elapsed.as_secs_f64().max(0.001);

        if !params.quiet {
            style::progress_line(n, rate / 1_000_000.0, elapsed.as_secs_f64());
        }

        // Save checkpoint every ~30s (≈20 ticks).
        checkpoint_tick += 1;
        if checkpoint_tick % 20 == 0 {
            checkpoint::save(&checkpoint_params, n, elapsed);
        }
    }

    // Final newline after progress line.
    if !params.quiet {
        eprintln!();
    }

    // ── Join ────────────────────────────────────────────────────────
    for h in handles {
        h.join()
            .map_err(|_| Error::ThreadPool("worker panicked".into()))?;
    }

    let elapsed = start.elapsed();
    let guard = results.lock().unwrap();
    if guard.is_empty() {
        Err(Error::ThreadPool("no result found – unexpected".into()))
    } else {
        Ok((guard.clone(), elapsed))
    }
}

// -----------------------------------------------------------------------
// Worker
// -----------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn worker(
    _pattern: &str,
    cmp_pat: &str,
    re: &Option<Regex>,
    match_mode: Arc<MatchMode>,
    addr_type: AddressType,
    case_insensitive: bool,
    compressed: bool,
    network: Network,
    found_count: Arc<AtomicU64>,
    counter: Arc<AtomicU64>,
    results: Arc<Mutex<Vec<FoundResult>>>,
    target_count: usize,
    strip_prefix: bool,
) {
    let secp = Secp256k1::new();
    let tweak_one = Scalar::ONE;

    'restart: loop {
        if found_count.load(Ordering::Relaxed) as usize >= target_count {
            return;
        }

        // ── Random starting point ───────────────────────────────────
        let mut sk_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut sk_bytes);
        let mut sk = match SecretKey::from_slice(&sk_bytes) {
            Ok(k) => k,
            Err(_) => continue, // ≈0 or ≥n → retry
        };
        let mut pk = bitcoin::secp256k1::PublicKey::from_secret_key(&secp, &sk);

        for _ in 0..BATCH_SIZE {
            if found_count.load(Ordering::Relaxed) as usize >= target_count {
                return;
            }

            // Derive address string (fast path – avoids Address object overhead).
            let addr_str = match addr_type {
                AddressType::Legacy => {
                    let pk_bytes = if compressed {
                        pk.serialize().to_vec() // 33 bytes
                    } else {
                        pk.serialize_uncompressed().to_vec() // 65 bytes
                    };
                    addr::p2pkh_address_fast(&pk_bytes)
                }
                AddressType::P2sh => {
                    let pk_bytes = pk.serialize(); // always compressed for P2SH
                    addr::p2sh_wpkh_address_fast(&pk_bytes)
                }
                _ => {
                    let a = match derive_single(&secp, &sk, compressed, network, addr_type) {
                        Ok(v) => v,
                        Err(_) => continue 'restart,
                    };
                    a.to_string()
                }
            };

            let n = counter.fetch_add(1, Ordering::Relaxed) + 1;

            // Match check using the configured match mode.
            let is_match = is_match(
                &addr_str,
                cmp_pat,
                *match_mode,
                case_insensitive,
                re.as_ref(),
                Some(addr_type),
                strip_prefix,
            );

            if is_match {
                let wif = wif::format_wif(&sk, compressed, network);
                let found_result = FoundResult {
                    address: addr_str,
                    wif,
                    total_attempts: n,
                    mnemonic_phrase: None,
                    derivation_path: None,
                };
                let mut list = results.lock().unwrap();
                list.push(found_result);
                found_count.fetch_add(1, Ordering::Relaxed);
                // Drop the lock before checking if we're done.
                drop(list);

                if found_count.load(Ordering::Relaxed) as usize >= target_count {
                    return;
                }
                // Continue walking forward to find more matches.
            }

            // Walk forward: sk += 1, pk += G.
            sk = match sk.add_tweak(&tweak_one) {
                Ok(k) => k,
                Err(_) => continue 'restart,
            };
            pk = match pk.add_exp_tweak(&secp, &tweak_one) {
                Ok(p) => p,
                Err(_) => continue 'restart,
            };
        }
        // Batch exhausted → re-randomise at 'restart.
    }
}

// -----------------------------------------------------------------------
// BIP39+BIP32 worker – checks ONLY address-index 0, generating a fresh
// mnemonic for every attempt.  This is much slower than the incremental
// tweaking approach but guarantees that every checked key corresponds to
// a valid BIP39 mnemonic at a standard HD-wallet derivation path, so the
// user can import the mnemonic into any BIP39/BIP32 wallet and the FIRST
// address they see IS the vanity address.
// -----------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn worker_bip32(
    _pattern: &str,
    cmp_pat: &str,
    re: &Option<Regex>,
    match_mode: Arc<MatchMode>,
    addr_type: AddressType,
    case_insensitive: bool,
    network: Network,
    found_count: Arc<AtomicU64>,
    counter: Arc<AtomicU64>,
    results: Arc<Mutex<Vec<FoundResult>>>,
    bip39_words: usize,
    target_count: usize,
    strip_prefix: bool,
) {
    let secp = Secp256k1::new();

    // BIP44 / BIP49 / BIP84 / BIP86 purpose per address type.
    let purpose = match addr_type {
        AddressType::Legacy => 44,
        AddressType::P2sh => 49,
        AddressType::Segwit => 84,
        AddressType::Taproot => 86,
    };
    // Full path for address_index = 0:
    //   m/purpose'/0'/0'/0/0
    let full_path_str = format!("m/{}'/0'/0'/0/0", purpose);
    let full_path: DerivationPath = full_path_str
        .parse()
        .expect("static BIP32 path is always valid");

    // Map word count to entropy bytes.
    let entropy_len = match bip39_words {
        12 => 16,
        15 => 20,
        18 => 24,
        21 => 28,
        _ => 32,
    };

    loop {
        if found_count.load(Ordering::Relaxed) as usize >= target_count {
            return;
        }

        // ── Generate a fresh BIP39 mnemonic ─────────────────────────
        let mut entropy = vec![0u8; entropy_len];
        OsRng.fill_bytes(&mut entropy);
        let mnemonic = match Mnemonic::from_entropy(&entropy) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let seed = mnemonic.to_seed("");
        let master = match Xpriv::new_master(network, &seed) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // ── Derive the key at the standard HD path with index 0 ─────
        let child = match master.derive_priv(&secp, &full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let addr = match derive_single(&secp, &child.private_key, true, network, addr_type) {
            Ok(a) => a,
            Err(_) => continue,
        };
        let addr_str = addr.to_string();

        // Match check using the configured match mode.
        let is_match = is_match(
            &addr_str,
            cmp_pat,
            *match_mode,
            case_insensitive,
            re.as_ref(),
            Some(addr_type),
            strip_prefix,
        );

        let n = counter.fetch_add(1, Ordering::Relaxed) + 1;

        if is_match {
            let wif = wif::format_wif(&child.private_key, true, network);
            let found_result = FoundResult {
                address: addr_str,
                wif,
                total_attempts: n,
                mnemonic_phrase: Some(mnemonic.to_string()),
                derivation_path: Some(full_path_str.clone()),
            };
            let mut list = results.lock().unwrap();
            list.push(found_result);
            found_count.fetch_add(1, Ordering::Relaxed);
            drop(list);

            if found_count.load(Ordering::Relaxed) as usize >= target_count {
                return;
            }
        }
        // Not a match → generate a brand-new mnemonic and try again.
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_is_match_prefix() {
        assert!(is_match(
            "1ABCDef",
            "1ABC",
            MatchMode::Prefix,
            false,
            None,
            None,
            false
        ));
        assert!(!is_match(
            "1ABCDef",
            "1XYZ",
            MatchMode::Prefix,
            false,
            None,
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_prefix_case_insensitive() {
        // Pattern must already be lowercased when case_insensitive=true
        // (the search() function pre-computes cmp_pat this way).
        assert!(is_match(
            "1ABCDef",
            "1abc",
            MatchMode::Prefix,
            true,
            None,
            None,
            false
        ));
        assert!(is_match(
            "1ABCDef",
            "1abcd",
            MatchMode::Prefix,
            true,
            None,
            None,
            false
        ));
        assert!(!is_match(
            "1ABCDef",
            "1xyz",
            MatchMode::Prefix,
            true,
            None,
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_suffix() {
        assert!(is_match(
            "1ABCDef",
            "Def",
            MatchMode::Suffix,
            false,
            None,
            None,
            false
        ));
        assert!(!is_match(
            "1ABCDef",
            "abc",
            MatchMode::Suffix,
            false,
            None,
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_suffix_case_insensitive() {
        assert!(is_match(
            "1ABCDef",
            "def",
            MatchMode::Suffix,
            true,
            None,
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_anywhere() {
        assert!(is_match(
            "1ABCDef",
            "BCD",
            MatchMode::Anywhere,
            false,
            None,
            None,
            false
        ));
        assert!(is_match(
            "1ABCDef",
            "1AB",
            MatchMode::Anywhere,
            false,
            None,
            None,
            false
        ));
        assert!(is_match(
            "1ABCDef",
            "Def",
            MatchMode::Anywhere,
            false,
            None,
            None,
            false
        ));
        assert!(!is_match(
            "1ABCDef",
            "XYZ",
            MatchMode::Anywhere,
            false,
            None,
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_regex() {
        let re = Regex::new("^1[A-Z]{3}").unwrap();
        assert!(is_match(
            "1ABCDef",
            "",
            MatchMode::Regex,
            false,
            Some(&re),
            None,
            false
        ));
        assert!(!is_match(
            "1abcDef",
            "",
            MatchMode::Regex,
            false,
            Some(&re),
            None,
            false
        ));

        let re2 = Regex::new("pizza$").unwrap();
        assert!(is_match(
            "bc1qpizza",
            "",
            MatchMode::Regex,
            false,
            Some(&re2),
            None,
            false
        ));
        assert!(!is_match(
            "bc1qpizzz",
            "",
            MatchMode::Regex,
            false,
            Some(&re2),
            None,
            false
        ));
    }

    #[test]
    fn test_is_match_regex_with_case_insensitive() {
        // case_insensitive should have no effect on regex mode
        let re = Regex::new("^1[A-Z]{3}").unwrap();
        assert!(is_match(
            "1ABCxyz",
            "",
            MatchMode::Regex,
            true,
            Some(&re),
            None,
            false
        ));
        assert!(!is_match(
            "1abcxyz",
            "",
            MatchMode::Regex,
            true,
            Some(&re),
            None,
            false
        ));
    }
}
