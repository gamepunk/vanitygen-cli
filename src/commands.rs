//! Command implementations for the vanitygen binary.
//!
//! Each top-level subcommand (`search`, `verify`, `address`, `benchmark`,
//! `mnemonic`) has a corresponding public function here, keeping `main.rs`
//! slim and focused on CLI dispatch.

use std::process;

use bitcoin::Network;
use clap::Parser;

use crate::address;
use crate::cli::{
    self, parse_address_type, resolve_match_mode, AddressType, Cli, CliCommand, MatchMode,
};
use crate::config;
use crate::error::Error;
use crate::log;
use crate::notify;
use crate::search;
use crate::style;
use crate::wif;

// ---------------------------------------------------------------------------
// CLI entry point — called from main.rs
// ---------------------------------------------------------------------------

/// Parse CLI arguments and dispatch to the appropriate command.
pub fn entry() {
    let cfg = config::Config::load();

    // Backward-compatible argument rewriting.
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
            count,
            no_strip_prefix,
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
                    *count,
                )
            } else if let Some(pat) = prefix {
                let strip_prefix = !no_strip_prefix;
                let sc = SearchConfig {
                    pattern: pat,
                    addr_type: *address_type,
                    case_insensitive: *case_insensitive,
                    compressed: !*uncompressed,
                    network: Network::Bitcoin,
                    num_threads: *threads,
                    use_bip32: *mnemonic,
                    quiet: *quiet,
                    match_mode,
                    bip39_words: *words,
                    target_count: *count,
                    output_file: output_file.as_deref(),
                    bark_key: bark.as_deref(),
                    strip_prefix,
                };
                run_search(&cfg, sc)
            } else {
                Err(Error::Other(
                    "Either a pattern or --input-file is required.".into(),
                ))
            }
        }
        CliCommand::Verify { wif } => crate::verify::run(wif),
        CliCommand::Address { wif } => run_address(wif),
        CliCommand::Benchmark => crate::benchmark::run(),
        CliCommand::Mnemonic { words } => run_mnemonic(*words),
    };

    if let Err(e) = result {
        eprintln!("错误: {e}");
        process::exit(1);
    }
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
// SearchConfig — bundles all search parameters to avoid long argument lists
// ---------------------------------------------------------------------------

/// All parameters needed to run a vanity address search.
#[derive(Debug, Clone)]
pub struct SearchConfig<'a> {
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
    pub output_file: Option<&'a str>,
    pub bark_key: Option<&'a str>,
    pub strip_prefix: bool,
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Run a vanity address search and display results.
pub fn run_search(cfg: &config::Config, sc: SearchConfig) -> Result<(), Error> {
    // Validate the pattern for the chosen address type (only for Prefix mode).
    if sc.match_mode == MatchMode::Prefix {
        if let Err(msg) = cli::validate_prefix(sc.pattern, sc.addr_type, sc.strip_prefix) {
            return Err(Error::InvalidPrefix(msg));
        }
    }

    // ── Self-test ───────────────────────────────────────────────────
    crate::self_test::run()?;

    // ── Checkpoint ──────────────────────────────────────────────────
    if let Some(ref cp) = crate::checkpoint::load() {
        crate::checkpoint::print_and_confirm(cp);
    }
    log::info(&format!(
        "开始搜索: pattern={}, mode={:?}, type={}, case_insensitive={}, threads={}",
        sc.pattern,
        sc.match_mode,
        sc.addr_type.label(),
        sc.case_insensitive,
        sc.num_threads,
    ));

    // ── Search info ─────────────────────────────────────────────────
    if !sc.quiet {
        style::header("Searching");
        style::kv("pattern", sc.pattern);
        style::kv("mode", &format!("{:?}", sc.match_mode));
        style::kv("type", sc.addr_type.label());
        style::kv("threads", &sc.num_threads.to_string());
        if sc.use_bip32 {
            style::kv("source", "BIP39+BIP32");
            style::kv("words", &sc.bip39_words.to_string());
        }
        if sc.target_count > 1 {
            style::kv("count", &sc.target_count.to_string());
        }
        eprintln!();
    }

    // ── Search ──────────────────────────────────────────────────────
    let (results, elapsed) = search::search(search::SearchParams {
        pattern: sc.pattern,
        addr_type: sc.addr_type,
        case_insensitive: sc.case_insensitive,
        compressed: sc.compressed,
        network: sc.network,
        num_threads: sc.num_threads,
        use_bip32: sc.use_bip32,
        quiet: sc.quiet,
        match_mode: sc.match_mode,
        bip39_words: sc.bip39_words,
        target_count: sc.target_count,
        strip_prefix: sc.strip_prefix,
    })?;

    // ── Clear checkpoint + write log ───────────────────────────────
    crate::checkpoint::clear();

    let secp = bitcoin::secp256k1::Secp256k1::new();
    let total_attempts: u64 = results.iter().map(|r| r.total_attempts).max().unwrap_or(0);

    log::info(&format!(
        "找到! pattern={}, matches={}, attempts={}, elapsed={:.2}s",
        sc.pattern,
        results.len(),
        total_attempts,
        elapsed.as_secs_f64(),
    ));

    // ── Send notification (only for first match) ───────────────────
    if let Some(bk) = notify::resolve_key(sc.bark_key, cfg.bark_key.as_deref()) {
        if let Some(first) = results.first() {
            let _ = notify::send_bark(
                &bk,
                &format!("🎯 {} vanity address(es) found!", results.len()),
                &format!(
                    "First: {}\nTotal attempts: {}\nElapsed: {:.1}s",
                    first.address,
                    total_attempts,
                    elapsed.as_secs_f64()
                ),
            );
        }
    }

    // ── Display results ────────────────────────────────────────────
    if !sc.quiet {
        style::header(&format!(
            "Found {} vanity address{}",
            results.len(),
            if results.len() > 1 { "es" } else { "" }
        ));
    }

    for (i, found) in results.iter().enumerate() {
        if results.len() > 1 {
            println!();
            style::header(&format!("Match #{}", i + 1));
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

        // ── Wallet addresses for this match ────────────────────────
        if let Some(ref phrase) = found.mnemonic_phrase {
            let wallet_addrs = derive_wallet_addresses(phrase, 0, sc.network)?;
            style::header("Wallet addresses (index 0)");
            println!("{}", wallet_addrs);
            println!();
            if i == results.len() - 1 {
                style::warning(
                    "Import the mnemonic into any BIP39 wallet. The above addresses will match exactly.",
                );
            }
        } else {
            let info = wif::parse_wif(&found.wif)?;
            let all_addrs =
                address::derive_all(&secp, &info.private_key.inner, info.compressed, sc.network)?;
            style::header("Same-key addresses");
            style::result_line("Legacy (P2PKH)", &all_addrs.legacy.to_string());
            style::result_line("Nested SegWit (P2SH)", &all_addrs.p2sh_segwit.to_string());
            style::result_line(
                "Native SegWit (P2WPKH)",
                &all_addrs.native_segwit.to_string(),
            );
            style::result_line("Taproot (P2TR)", &all_addrs.taproot.to_string());
        }
    }

    println!();
    style::kv("total attempts", &total_attempts.to_string());
    style::kv("elapsed", &format!("{:.2}s", elapsed.as_secs_f64()));

    if results.iter().all(|r| r.mnemonic_phrase.is_none()) {
        println!();
        style::warning("Move funds immediately. Clear terminal history.");
    }

    // ── Auto-save to ~/.config/vanitygen/results.txt ──────────────
    let results_dir = dirs_config_path().join("vanitygen");
    let auto_path = results_dir.join("results.txt");
    if let Err(e) = auto_save_results(
        &auto_path,
        sc.pattern,
        &results,
        elapsed,
        sc.match_mode,
        &secp,
        sc.network,
    ) {
        log::info(&format!(
            "Failed to save results to {}: {e}",
            auto_path.display()
        ));
    }

    // ── Write to output file if requested ──────────────────────────
    if let Some(path) = sc.output_file {
        for found in &results {
            append_result(path, sc.pattern, found, elapsed, sc.match_mode)?;
        }
        if !sc.quiet {
            println!();
            style::success(&format!("{} result(s) appended to {}", results.len(), path));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Batch search (input file)
// ---------------------------------------------------------------------------

/// Process patterns from an input file (one per line).
///
/// Each line can optionally include inline flags after the pattern.
/// Inline flags override CLI defaults for that specific line.
#[allow(clippy::too_many_arguments)]
pub fn run_search_file(
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
    count: usize,
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
        let (pat, line_match_mode, line_addr_type, line_case_insensitive, line_count) =
            parse_line_flags(
                line,
                cli_match_mode,
                cli_addr_type,
                cli_case_insensitive,
                count,
            )?;

        if !quiet {
            println!();
            let label = if line_count > 1 {
                format!("{}/{} (count={})", i + 1, total, line_count)
            } else {
                format!("{}/{}", i + 1, total)
            };
            style::header(&format!("[{}] Searching for: {}", label, pat));
            if line_match_mode != cli_match_mode {
                style::kv("mode", &format!("{:?}", line_match_mode));
            }
            if line_addr_type != cli_addr_type {
                style::kv("type", line_addr_type.label());
            }
        }

        let sc = SearchConfig {
            pattern: &pat,
            addr_type: line_addr_type,
            case_insensitive: line_case_insensitive,
            compressed: !uncompressed,
            network: Network::Bitcoin,
            num_threads: threads,
            use_bip32,
            quiet,
            match_mode: line_match_mode,
            bip39_words,
            target_count: line_count,
            output_file,
            bark_key,
            strip_prefix: true,
        };

        match run_search(cfg, sc) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("  错误 (skipping): {e}");
                log::info(&format!("Skipping pattern '{pat}': {e}"));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Parse input-file line flags
// ---------------------------------------------------------------------------

/// Parse a single input-file line for optional inline flags.
///
/// Supported inline flags (inline overrides CLI default):
/// - `-p` / `--prefix`         → Prefix mode
/// - `-s` / `--suffix`         → Suffix mode
/// - `-a` / `--anywhere`       → Anywhere mode
/// - `-r` / `--regex`          → Regex mode
/// - `-t <type>` / `--address-type <type>` → address type
/// - `-i` / `--case-insensitive` → case insensitive
/// - `-n <N>` / `--count <N>`  → number of addresses to find
///
/// Returns `(pattern, match_mode, address_type, case_insensitive, count)`.
pub fn parse_line_flags(
    line: &str,
    default_mode: MatchMode,
    default_addr: AddressType,
    default_case: bool,
    default_count: usize,
) -> Result<(String, MatchMode, AddressType, bool, usize), Error> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(Error::Other("Empty line in input file".into()));
    }

    let pattern = tokens[0].to_string();
    let mut mode = default_mode;
    let mut addr = default_addr;
    let mut case_insensitive = default_case;
    let mut count = default_count;
    let mut i = 1;

    while i < tokens.len() {
        match tokens[i] {
            "-p" | "--prefix" => mode = MatchMode::Prefix,
            "-s" | "--suffix" => mode = MatchMode::Suffix,
            "-a" | "--anywhere" => mode = MatchMode::Anywhere,
            "-r" | "--regex" => mode = MatchMode::Regex,
            "-i" | "--case-insensitive" => case_insensitive = true,
            "-n" | "--count" => {
                i += 1;
                if i >= tokens.len() {
                    return Err(Error::Other(
                        "Missing value for -n/--count in input file".into(),
                    ));
                }
                count = tokens[i].parse::<usize>().map_err(|_| {
                    Error::Other(format!(
                        "Invalid count '{}' in input file line (expected a number)",
                        tokens[i]
                    ))
                })?;
            }
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

    Ok((pattern, mode, addr, case_insensitive, count))
}

// ---------------------------------------------------------------------------
// Auto-save results
// ---------------------------------------------------------------------------

/// Get the standard config directory (`~/.config/` or `$XDG_CONFIG_HOME`).
fn dirs_config_path() -> std::path::PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home).join(".config")
        })
}

/// Auto-save search results to a file with rich fields.
fn auto_save_results(
    path: &std::path::Path,
    pattern: &str,
    results: &[search::FoundResult],
    elapsed: std::time::Duration,
    match_mode: MatchMode,
    secp: &bitcoin::secp256k1::Secp256k1<bitcoin::secp256k1::All>,
    network: Network,
) -> Result<(), Error> {
    use std::fs::{create_dir_all, OpenOptions};
    use std::io::Write;

    // Ensure directory exists.
    if let Some(parent) = path.parent() {
        create_dir_all(parent).map_err(|e| {
            Error::Other(format!(
                "Cannot create results directory '{}': {e}",
                parent.display()
            ))
        })?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| {
            Error::Other(format!(
                "Cannot open results file '{}': {e}",
                path.display()
            ))
        })?;

    let timestamp = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );

    for found in results {
        writeln!(file, "---")?;
        writeln!(file, "created_at: {}", timestamp)?;
        writeln!(file, "pattern: {pattern}")?;
        writeln!(file, "mode: {:?}", match_mode)?;
        writeln!(file, "address: {}", found.address)?;
        writeln!(file, "wif: {}", found.wif)?;

        // Parse WIF to derive public key and all address types.
        if let Ok(info) = wif::parse_wif(&found.wif) {
            let pk = bitcoin::secp256k1::PublicKey::from_secret_key(secp, &info.private_key.inner);
            writeln!(file, "public_key: {}", pk)?;

            if let Ok(addrs) =
                address::derive_all(secp, &info.private_key.inner, info.compressed, network)
            {
                writeln!(file, "legacy_p2pkh: {}", addrs.legacy)?;
                writeln!(file, "p2sh_segwit: {}", addrs.p2sh_segwit)?;
                writeln!(file, "native_segwit: {}", addrs.native_segwit)?;
                writeln!(file, "taproot: {}", addrs.taproot)?;
            }
        }

        if let Some(ref phrase) = found.mnemonic_phrase {
            writeln!(file, "mnemonic: {phrase}")?;
        }
        if let Some(ref path) = found.derivation_path {
            writeln!(file, "derivation_path: {path}")?;
        }

        writeln!(file, "attempts: {}", found.total_attempts)?;
        writeln!(file, "elapsed_secs: {:.2}", elapsed.as_secs_f64())?;
        writeln!(file)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Result file output (-o / --output-file)
// ---------------------------------------------------------------------------

/// Append a single search result to a file in a structured format.
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

// ---------------------------------------------------------------------------
// Address / Mnemonic display commands
// ---------------------------------------------------------------------------

/// Derive and display all four address types from a WIF.
pub fn run_address(wif_str: &str) -> Result<(), Error> {
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
pub fn run_mnemonic(words: usize) -> Result<(), Error> {
    let entropy_bytes = cli::word_count_to_entropy_bytes(words).map_err(Error::Other)?;
    let result = crate::mnemonic::generate_random(entropy_bytes)?;

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

    style::warning(&format!(
        "Write down these {} words. Keep them offline. Anyone with this phrase can steal your funds.",
        words
    ));
    style::warning("Test with a small amount before depositing significant funds.");

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
                AddressType::Legacy
            } else if purpose == 49 {
                AddressType::P2sh
            } else if purpose == 84 {
                AddressType::Segwit
            } else {
                AddressType::Taproot
            },
        )?;
        lines.push(format!("  {label:<24} {}  (path: {path_str})", addr));
    }

    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::MatchMode;

    #[test]
    fn test_parse_line_flags_plain_pattern() {
        let (pat, mode, addr, ci, cnt) =
            parse_line_flags("1Bitcoin", MatchMode::Prefix, AddressType::Legacy, false, 1).unwrap();
        assert_eq!(pat, "1Bitcoin");
        assert_eq!(mode, MatchMode::Prefix);
        assert_eq!(addr, AddressType::Legacy);
        assert!(!ci);
        assert_eq!(cnt, 1);
    }

    #[test]
    fn test_parse_line_flags_suffix() {
        let (pat, mode, addr, ci, cnt) =
            parse_line_flags("pizza -s", MatchMode::Prefix, AddressType::Legacy, false, 1).unwrap();
        assert_eq!(pat, "pizza");
        assert_eq!(mode, MatchMode::Suffix);
        assert_eq!(addr, AddressType::Legacy);
        assert!(!ci);
        assert_eq!(cnt, 1);
    }

    #[test]
    fn test_parse_line_flags_anywhere_and_type() {
        let (pat, mode, addr, ci, cnt) = parse_line_flags(
            "ninja -a -t segwit",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        )
        .unwrap();
        assert_eq!(pat, "ninja");
        assert_eq!(mode, MatchMode::Anywhere);
        assert_eq!(addr, AddressType::Segwit);
        assert!(!ci);
        assert_eq!(cnt, 1);
    }

    #[test]
    fn test_parse_line_flags_regex_insensitive() {
        let (pat, mode, _addr, ci, cnt) = parse_line_flags(
            "^1A.*T$ -r -i",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        )
        .unwrap();
        assert_eq!(pat, "^1A.*T$");
        assert_eq!(mode, MatchMode::Regex);
        assert!(ci);
        assert_eq!(cnt, 1);
    }

    #[test]
    fn test_parse_line_flags_with_count() {
        let (pat, _mode, _addr, _ci, cnt) = parse_line_flags(
            "test -n 5",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        )
        .unwrap();
        assert_eq!(pat, "test");
        assert_eq!(cnt, 5);
    }

    #[test]
    fn test_parse_line_flags_with_count_override() {
        let (pat, _mode, _addr, _ci, cnt) = parse_line_flags(
            "test --count 10",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        )
        .unwrap();
        assert_eq!(pat, "test");
        assert_eq!(cnt, 10);
    }

    #[test]
    fn test_parse_line_flags_unknown_flag() {
        let result = parse_line_flags(
            "test --unknown",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_flags_missing_type_value() {
        let result = parse_line_flags("test -t", MatchMode::Prefix, AddressType::Legacy, false, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_flags_missing_count_value() {
        let result = parse_line_flags("test -n", MatchMode::Prefix, AddressType::Legacy, false, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_line_flags_invalid_type() {
        let result = parse_line_flags(
            "test -t invalid",
            MatchMode::Prefix,
            AddressType::Legacy,
            false,
            1,
        );
        assert!(result.is_err());
    }
}
