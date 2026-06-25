//! Bitcoin Vanity Address Generator — professional CPU multi-threaded edition.
//!
//! All cryptographic primitives are delegated to the audited
//! [`rust-bitcoin`](https://github.com/rust-bitcoin/rust-bitcoin) crate and its
//! underlying [`libsecp256k1`](https://github.com/bitcoin-core/secp256k1) C library.
//!
//! ## Usage
//!
//! ```text
//! vanity <prefix>                 # search (legacy syntax)
//! vanity search <prefix> [options]     # search (explicit)
//! vanity verify <WIF>                  # verify a private key
//! vanity address <WIF>                 # derive all 4 address types
//! vanity benchmark                     # throughput benchmark
//! ```

pub mod address;
pub mod benchmark;
pub mod checkpoint;
pub mod cli;
pub mod config;
pub mod error;
pub mod log;
pub mod mnemonic;
pub mod notify;
pub mod search;
pub mod self_test;
pub mod style;
pub mod verify;
pub mod wif;

use std::process;

use bitcoin::Network;
use clap::Parser;

use cli::{
    parse_address_type, resolve_match_mode, validate_prefix, AddressType, Cli, CliCommand,
    MatchMode,
};
use error::Error;

fn main() {
    // ── Load config ────────────────────────────────────────────────
    let cfg = config::Config::load();

    // ── Backward-compatible argument rewriting ─────────────────────
    let raw: Vec<String> = std::env::args().collect();
    let args: Vec<String> = if raw.len() >= 2 && !is_subcommand(&raw[1]) && !raw[1].starts_with('-')
    {
        let mut v = vec![raw[0].clone(), "search".to_string()];
        v.extend(raw[1..].iter().cloned());
        v
    } else {
        raw
    };

    let cli = Cli::parse_from(&args);

    let result = match &cli.command {
        CliCommand::Search {
            prefix,
            address_type,
            case_insensitive,
            mnemonic,
            uncompressed,
            threads,
            match_prefix,
            suffix,
            anywhere,
            regex,
            quiet,
            bark,
            input_file,
            output_file,
            words,
        } => {
            let match_mode = resolve_match_mode(*match_prefix, *suffix, *anywhere, *regex);
            if let Some(ifile) = input_file {
                run_search_file(
                    &cfg,
                    ifile,
                    output_file.as_deref(),
                    *address_type,
                    *case_insensitive,
                    *uncompressed,
                    *mnemonic,
                    *threads,
                    *quiet,
                    bark.as_deref(),
                    match_mode,
                    *words,
                )
            } else if let Some(pat) = prefix {
                run_search(
                    &cfg,
                    pat,
                    output_file.as_deref(),
                    *address_type,
                    *case_insensitive,
                    *uncompressed,
                    *mnemonic,
                    *threads,
                    *quiet,
                    bark.as_deref(),
                    match_mode,
                    *words,
                )
            } else {
                // Should not happen due to clap required_unless_present.
                Err(Error::Other(
                    "Either a pattern or --input-file is required.".into(),
                ))
            }
        }
        CliCommand::Verify { wif } => verify::run(wif),
        CliCommand::Address { wif } => run_address(wif),
        CliCommand::Benchmark => benchmark::run(),
        CliCommand::Mnemonic { words } => run_mnemonic(*words),
    };

    if let Err(e) = result {
        eprintln!("错误: {e}");
        process::exit(1);
    }
}

// ── Subcommand implementations ──────────────────────────────────────────

/// Search for a vanity address.
#[allow(clippy::too_many_arguments)]
fn run_search(
    cfg: &config::Config,
    pattern: &str,
    output_file: Option<&str>,
    addr_type: AddressType,
    case_insensitive: bool,
    uncompressed: bool,
    use_bip32: bool,
    threads: usize,
    quiet: bool,
    bark_key: Option<&str>,
    match_mode: MatchMode,
    bip39_words: usize,
) -> Result<(), Error> {
    // Validate the pattern for the chosen address type (only for Prefix mode).
    if match_mode == MatchMode::Prefix {
        if let Err(msg) = validate_prefix(pattern, addr_type) {
            return Err(Error::InvalidPrefix(msg));
        }
    }

    let network = Network::Bitcoin;
    let compressed = !uncompressed;

    // ── Self-test ───────────────────────────────────────────────────
    self_test::run()?;
    if !quiet {
        style::success("Self-test passed");
    }

    // ── Checkpoint ──────────────────────────────────────────────────
    if let Some(ref cp) = checkpoint::load() {
        checkpoint::print_and_confirm(cp);
    }
    log::info(&format!(
        "开始搜索: pattern={pattern}, mode={:?}, type={}, case_insensitive={case_insensitive}, threads={threads}",
        match_mode,
        addr_type.label(),
    ));

    // ── Search info ─────────────────────────────────────────────────
    if !quiet {
        style::header("Searching");
        style::kv("pattern", pattern);
        style::kv("mode", &format!("{:?}", match_mode));
        style::kv("type", addr_type.label());
        style::kv("threads", &threads.to_string());
        if use_bip32 {
            style::kv("source", "BIP39+BIP32");
            style::kv("words", &bip39_words.to_string());
        }
        eprintln!();
    }

    // ── Search ──────────────────────────────────────────────────────
    let (found, elapsed) = search::search(
        pattern,
        addr_type,
        case_insensitive,
        compressed,
        network,
        threads,
        use_bip32,
        quiet,
        match_mode,
        bip39_words,
    )?;

    // ── Clear checkpoint + write log ───────────────────────────────
    checkpoint::clear();
    log::info(&format!(
        "找到! pattern={}, address={}, attempts={}, elapsed={:.2}s",
        pattern,
        found.address,
        found.total_attempts,
        elapsed.as_secs_f64(),
    ));

    let info = wif::parse_wif(&found.wif)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();

    // ── Send notification (if Bark key provided) ──────────────────
    if let Some(bk) = notify::resolve_key(bark_key, cfg.bark_key.as_deref()) {
        let _ = notify::send_bark(
            &bk,
            "🎯 Vanity address found!",
            &format!(
                "Address: {}\nElapsed: {:.1}s",
                found.address,
                elapsed.as_secs_f64()
            ),
        );
    }

    if !quiet {
        style::header("Found vanity address");
    }

    style::result_line("Address", &found.address);
    style::result_line("WIF", &found.wif);

    if let Some(ref phrase) = found.mnemonic_phrase {
        println!();
        style::header("BIP39 Mnemonic");
        println!("  {}", phrase);
    }
    if let Some(ref path) = found.derivation_path {
        style::kv("derivation path", path);
    }

    println!();
    style::kv("attempts", &found.total_attempts.to_string());
    style::kv("elapsed", &format!("{:.2}s", elapsed.as_secs_f64()));
    println!();

    // ── Wallet addresses ───────────────────────────────────────────
    if let Some(ref phrase) = found.mnemonic_phrase {
        let wallet_addrs = derive_wallet_addresses(phrase, 0, network)?;
        style::header("Wallet addresses (index 0)");
        println!("{}", wallet_addrs);
        println!();
        style::warning(
            "Import the mnemonic into any BIP39 wallet. The above addresses will match exactly.",
        );
    } else {
        let all_addrs =
            address::derive_all(&secp, &info.private_key.inner, info.compressed, network)?;
        style::header("Same-key addresses");
        style::result_line("Legacy (P2PKH)", &all_addrs.legacy.to_string());
        style::result_line("Nested SegWit (P2SH)", &all_addrs.p2sh_segwit.to_string());
        style::result_line(
            "Native SegWit (P2WPKH)",
            &all_addrs.native_segwit.to_string(),
        );
        style::result_line("Taproot (P2TR)", &all_addrs.taproot.to_string());
    }

    println!();
    style::warning("Move funds immediately. Clear terminal history.");

    // ── Write to output file if requested ──────────────────────────
    if let Some(path) = output_file {
        append_result(path, pattern, &found, elapsed, match_mode)?;
        if !quiet {
            println!();
            style::success(&format!("Result appended to {}", path));
        }
    }

    Ok(())
}

/// Process patterns from an input file (one per line).
///
/// Each line can optionally include inline flags after the pattern:
/// ```text
/// 1Bitcoin              # uses CLI defaults
/// ninja -a -t segwit    # anywhere mode, segwit
/// pizza -s -i           # suffix mode, case-insensitive
/// ^1A.*T$ -r            # regex mode
/// ```
#[allow(clippy::too_many_arguments)]
fn run_search_file(
    cfg: &config::Config,
    input_file: &str,
    output_file: Option<&str>,
    cli_addr_type: AddressType,
    cli_case_insensitive: bool,
    uncompressed: bool,
    use_bip32: bool,
    threads: usize,
    quiet: bool,
    bark_key: Option<&str>,
    cli_match_mode: MatchMode,
    bip39_words: usize,
) -> Result<(), Error> {
    use std::fs;
    use std::io::{BufRead, BufReader};

    let file = fs::File::open(input_file)
        .map_err(|e| Error::Other(format!("Cannot open input file '{}': {e}", input_file)))?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();

    if lines.is_empty() {
        return Err(Error::Other(format!(
            "No patterns found in input file '{}'",
            input_file
        )));
    }

    if !quiet {
        style::header("Batch search");
        style::kv("patterns", &lines.len().to_string());
        style::kv("mode", &format!("{:?}", cli_match_mode));
        style::kv("type", cli_addr_type.label());
        style::kv("threads", &threads.to_string());
        eprintln!();
    }

    let total = lines.len();
    for (i, line) in lines.iter().enumerate() {
        // Parse line: extract pattern and optional inline flags.
        let (pat, line_match_mode, line_addr_type, line_case_insensitive) =
            parse_line_flags(line, cli_match_mode, cli_addr_type, cli_case_insensitive)?;

        if !quiet {
            println!();
            style::header(&format!("[{}/{}] Searching for: {}", i + 1, total, pat));
            if line_match_mode != cli_match_mode {
                style::kv("mode", &format!("{:?}", line_match_mode));
            }
            if line_addr_type != cli_addr_type {
                style::kv("type", line_addr_type.label());
            }
        }

        match run_search(
            cfg,
            &pat,
            output_file,
            line_addr_type,
            line_case_insensitive,
            uncompressed,
            use_bip32,
            threads,
            quiet,
            bark_key,
            line_match_mode,
            bip39_words,
        ) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("  错误 (skipping): {e}");
                log::info(&format!("Skipping pattern '{pat}': {e}"));
            }
        }
    }

    Ok(())
}

/// Parse a single input-file line for optional inline flags.
///
/// Supported inline flags:
/// - `-p` / `--prefix`         → Prefix mode
/// - `-s` / `--suffix`         → Suffix mode
/// - `-a` / `--anywhere`       → Anywhere mode
/// - `-r` / `--regex`          → Regex mode
/// - `-t <type>` / `--address-type <type>` → address type
/// - `-i` / `--case-insensitive` → case insensitive
///
/// Returns (pattern, match_mode, address_type, case_insensitive).
fn parse_line_flags(
    line: &str,
    default_mode: MatchMode,
    default_addr: AddressType,
    default_case: bool,
) -> Result<(String, MatchMode, AddressType, bool), Error> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(Error::Other("Empty line in input file".into()));
    }

    let pattern = tokens[0].to_string();
    let mut mode = default_mode;
    let mut addr = default_addr;
    let mut case_insensitive = default_case;
    let mut i = 1;

    while i < tokens.len() {
        match tokens[i] {
            "-p" | "--prefix" => mode = MatchMode::Prefix,
            "-s" | "--suffix" => mode = MatchMode::Suffix,
            "-a" | "--anywhere" => mode = MatchMode::Anywhere,
            "-r" | "--regex" => mode = MatchMode::Regex,
            "-i" | "--case-insensitive" => case_insensitive = true,
            "-t" | "--address-type" => {
                i += 1;
                if i >= tokens.len() {
                    return Err(Error::Other(
                        "Missing value for -t/--address-type in input file".into(),
                    ));
                }
                addr = parse_address_type(tokens[i]).map_err(|e| {
                    Error::Other(format!("Invalid address type in input file: {e}"))
                })?;
            }
            other => {
                return Err(Error::Other(format!(
                    "Unknown flag '{}' in input file line: {}",
                    other, line
                )));
            }
        }
        i += 1;
    }

    Ok((pattern, mode, addr, case_insensitive))
}

/// Append a search result to a file in a structured format.
fn append_result(
    path: &str,
    pattern: &str,
    found: &search::FoundResult,
    elapsed: std::time::Duration,
    match_mode: MatchMode,
) -> Result<(), Error> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::Other(format!("Cannot open output file '{}': {e}", path)))?;

    writeln!(
        file,
        "---\npattern: {pattern}\nmode: {:?}\naddress: {}\nwif: {}\nattempts: {}\nelapsed: {:.2}s",
        match_mode,
        found.address,
        found.wif,
        found.total_attempts,
        elapsed.as_secs_f64(),
    )
    .map_err(|e| Error::Other(format!("Cannot write to output file '{}': {e}", path)))?;

    if let Some(ref phrase) = found.mnemonic_phrase {
        writeln!(file, "mnemonic: {phrase}")?;
    }
    if let Some(ref path) = found.derivation_path {
        writeln!(file, "derivation_path: {path}")?;
    }

    writeln!(file)?;
    Ok(())
}

/// Derive wallet-compatible addresses for all 4 standard BIP32 paths
/// (BIP44 / BIP49 / BIP84 / BIP86) at a given address index.
fn derive_wallet_addresses(phrase: &str, index: u32, network: Network) -> Result<String, Error> {
    use bip39::Mnemonic;
    use bitcoin::bip32::{DerivationPath, Xpriv};

    let mnemonic =
        Mnemonic::parse(phrase).map_err(|e| Error::InvalidWif(format!("mnemonic parse: {e}")))?;
    let seed = mnemonic.to_seed("");
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let master = Xpriv::new_master(network, &seed)?;

    // BIP44 / BIP49 / BIP84 / BIP86 at the given index.
    let configs = [
        (44, "Legacy (P2PKH)"),
        (49, "Nested SegWit (P2SH-P2WPKH)"),
        (84, "Native SegWit (P2WPKH)"),
        (86, "Taproot (P2TR)"),
    ];

    let mut lines = Vec::new();
    for &(purpose, label) in &configs {
        let path_str = format!("m/{}'/0'/0'/0/{index}", purpose);
        let path: DerivationPath = path_str.parse()?;
        let child = master.derive_priv(&secp, &path)?;
        let addr = address::derive_single(
            &secp,
            &child.private_key,
            true,
            network,
            if purpose == 44 {
                cli::AddressType::Legacy
            } else if purpose == 49 {
                cli::AddressType::P2sh
            } else if purpose == 84 {
                cli::AddressType::Segwit
            } else {
                cli::AddressType::Taproot
            },
        )?;
        lines.push(format!("  {label:<24} {}  (path: {path_str})", addr));
    }

    Ok(lines.join("\n"))
}

/// Derive and display all four address types from a WIF.
fn run_address(wif_str: &str) -> Result<(), Error> {
    let info = wif::parse_wif(wif_str)?;
    let secp = bitcoin::secp256k1::Secp256k1::new();
    let set = address::derive_all(
        &secp,
        &info.private_key.inner,
        info.compressed,
        info.network,
    )?;

    let net = match info.network {
        Network::Bitcoin => "Mainnet",
        Network::Testnet => "Testnet",
        Network::Signet => "Signet",
        Network::Regtest => "Regtest",
        _ => "Unknown",
    };

    style::header("Addresses from private key");
    style::kv("network", net);
    style::kv("compressed", &info.compressed.to_string());
    println!();
    style::header("Derived addresses");
    style::result_line("Legacy (P2PKH)", &set.legacy.to_string());
    style::result_line("Nested SegWit (P2SH)", &set.p2sh_segwit.to_string());
    style::result_line("Native SegWit (P2WPKH)", &set.native_segwit.to_string());
    style::result_line("Taproot (P2TR)", &set.taproot.to_string());
    println!();

    Ok(())
}

/// Generate a random BIP39 mnemonic and display all derived addresses.
fn run_mnemonic(words: usize) -> Result<(), Error> {
    let entropy_bytes = cli::word_count_to_entropy_bytes(words).map_err(Error::Other)?;
    let result = mnemonic::generate_random(entropy_bytes)?;

    let bits = entropy_bytes * 8;
    style::header(&format!("BIP39 Mnemonic ({} words, {}-bit)", words, bits));
    println!("  {}", result.phrase);
    println!();

    for p in &result.paths {
        style::header(p.label);
        style::kv("path", &p.path);
        style::kv("WIF", &p.wif);
        style::result_line("Legacy (P2PKH)", &p.legacy);
        style::result_line("Nested SegWit (P2SH)", &p.p2sh);
        style::result_line("Native SegWit (P2WPKH)", &p.segwit);
        style::result_line("Taproot (P2TR)", &p.taproot);
        println!();
    }

    style::warning(&format!("Write down these {} words. Keep them offline. Anyone with this phrase can steal your funds.", words));
    style::warning("Test with a small amount before depositing significant funds.");

    Ok(())
}

/// Check whether a string is a known subcommand name.
fn is_subcommand(s: &str) -> bool {
    matches!(
        s,
        "search"
            | "s"
            | "verify"
            | "v"
            | "address"
            | "a"
            | "addr"
            | "benchmark"
            | "b"
            | "bench"
            | "mnemonic"
            | "m"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_line_flags_plain_pattern() {
        let (pat, mode, addr, ci) =
            parse_line_flags("1Bitcoin", MatchMode::Prefix, AddressType::Legacy, false).unwrap();
        assert_eq!(pat, "1Bitcoin");
        assert_eq!(mode, MatchMode::Prefix);
        assert_eq!(addr, AddressType::Legacy);
        assert!(!ci);
    }

    #[test]
    fn test_parse_line_flags_suffix() {
        let (pat, mode, addr, ci) =
            parse_line_flags("pizza -s", MatchMode::Prefix, AddressType::Legacy, false).unwrap();
        assert_eq!(pat, "pizza");
        assert_eq!(mode, MatchMode::Suffix);
        assert_eq!(addr, AddressType::Legacy);
        assert!(!ci);
    }

    #[test]
    fn test_parse_line_flags_anywhere_and_type() {
        let (pat, mode, addr, ci) = parse_line_flags(
            "ninja -a -t segwit",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
        )
        .unwrap();
        assert_eq!(pat, "ninja");
        assert_eq!(mode, MatchMode::Anywhere);
        assert_eq!(addr, AddressType::Segwit);
        assert!(!ci);
    }

    #[test]
    fn test_parse_line_flags_regex_insensitive() {
        let (pat, mode, _addr, ci) = parse_line_flags(
            "^1A.*T$ -r -i",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
        )
        .unwrap();
        assert_eq!(pat, "^1A.*T$");
        assert_eq!(mode, MatchMode::Regex);
        assert!(ci);
    }

    #[test]
    fn test_parse_line_flags_unknown_flag() {
        let result =
            parse_line_flags("test --unknown", MatchMode::Prefix, AddressType::Legacy, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_flags_missing_type_value() {
        let result =
            parse_line_flags("test -t", MatchMode::Prefix, AddressType::Legacy, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_flags_invalid_type() {
        let result =
            parse_line_flags("test -t invalid", MatchMode::Prefix, AddressType::Legacy, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_subcommand() {
        assert!(is_subcommand("search"));
        assert!(is_subcommand("s"));
        assert!(is_subcommand("verify"));
        assert!(is_subcommand("mnemonic"));
        assert!(!is_subcommand(""));
        assert!(!is_subcommand("notacommand"));
    }
}
