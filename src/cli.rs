//! CLI argument parsing via `clap`.
//!
//! Supports four subcommands:
//! - **search** (default): find a vanity address matching a prefix.
//! - **verify**: parse a WIF and display its derived addresses.
//! - **address**: derive all four address types from a WIF.
//! - **benchmark**: measure key-generation throughput.

use clap::{Parser, Subcommand};

/// Bitcoin vanity address generator.
#[derive(Parser)]
#[command(
    name = "vanitygen",
    version,
    about = "Generate custom vanity Bitcoin addresses",
    long_about = None,
    after_help = "\
Run directly after build:
  cargo build --release
  ./target/release/vanitygen 1Bit

Install globally:
  cargo install vanitygen
  vanitygen 1Bit

Examples:
  vanitygen 1Bit                   Search Legacy prefix
  vanitygen bc1qninja -t segwit   Search SegWit prefix
  vanitygen verify <WIF>           Verify a private key

All commands accept -h (or --help) for more options.",
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand)]
pub enum CliCommand {
    /// Search for a vanity address matching the given prefix.
    #[command(aliases = ["s"])]
    Search {
        /// Address prefix to search for, e.g. "1Bit", "bc1qninja".
        #[arg(required_unless_present = "input_file")]
        prefix: Option<String>,

        /// Address type: legacy | p2sh | segwit | taproot.
        #[arg(long, short = 't', value_parser = parse_address_type, default_value = "legacy")]
        address_type: AddressType,

        /// Case-insensitive matching (faster, but address letter-case is random).
        #[arg(long, short = 'i')]
        case_insensitive: bool,

        /// Use BIP39+BIP32 derivation (slower, but outputs a mnemonic seed phrase).
        #[arg(long, short = 'm')]
        mnemonic: bool,

        /// Use uncompressed public key (Legacy / P2PKH only; ignored with --mnemonic).
        #[arg(long, short = 'u')]
        uncompressed: bool,

        /// Number of worker threads (default: all logical cores).
        #[arg(long, short = 'T', default_value_t = { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) })]
        threads: usize,

        /// Match pattern as prefix (default).
        #[arg(long, short = 'p', conflicts_with_all = ["suffix", "anywhere", "regex"])]
        match_prefix: bool,

        /// Match pattern as suffix.
        #[arg(long, short = 's', conflicts_with_all = ["match_prefix", "anywhere", "regex"])]
        suffix: bool,

        /// Match pattern anywhere in the address.
        #[arg(long, short = 'a', conflicts_with_all = ["match_prefix", "suffix", "regex"])]
        anywhere: bool,

        /// Match address using a regular expression.
        #[arg(long, short = 'r', conflicts_with_all = ["match_prefix", "suffix", "anywhere"])]
        regex: bool,

        /// Quiet mode: suppress progress output, only print final result.
        #[arg(long, short = 'q')]
        quiet: bool,

        /// Bark API key for iOS push notification (or set VANITY_BARK_KEY env).
        #[arg(long)]
        bark: Option<String>,

        /// Input file with patterns (one per line). Overrides positional PREFIX.
        #[arg(long)]
        input_file: Option<String>,

        /// Output file to write results (appended).
        #[arg(long, short = 'o')]
        output_file: Option<String>,
    },

    /// Parse a WIF private key and display its derived addresses.
    #[command(aliases = ["v"])]
    Verify {
        /// WIF private key, e.g. "Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12".
        wif: String,
    },

    /// Derive all four address types from a given WIF private key.
    #[command(aliases = ["a", "addr"])]
    Address {
        /// WIF private key.
        wif: String,
    },

    /// Run a throughput benchmark (keys / second).
    #[command(aliases = ["b", "bench"])]
    Benchmark,

    /// Generate a random BIP39 mnemonic (24 words) and derive all addresses.
    #[command(aliases = ["m"])]
    Mnemonic,
}

/// Supported address types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    Legacy,  // P2PKH  → 1…
    P2sh,    // P2SH-wrapped P2WPKH → 3…
    Segwit,  // Native SegWit P2WPKH → bc1q…
    Taproot, // P2TR → bc1p…
}
/// How to match the pattern against the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Prefix,
    Suffix,
    Anywhere,
    Regex,
}

/// Resolve match mode from CLI flags (default: Prefix).
pub fn resolve_match_mode(_match_prefix: bool, suffix: bool, anywhere: bool, regex: bool) -> MatchMode {
    if suffix { MatchMode::Suffix }
    else if anywhere { MatchMode::Anywhere }
    else if regex { MatchMode::Regex }
    else { MatchMode::Prefix }
}
fn parse_address_type(s: &str) -> Result<AddressType, String> {
    match s.to_lowercase().as_str() {
        "legacy" | "p2pkh" => Ok(AddressType::Legacy),
        "p2sh" | "p2sh-segwit" | "p2sh-p2wpkh" => Ok(AddressType::P2sh),
        "segwit" | "p2wpkh" | "native-segwit" | "bech32" => Ok(AddressType::Segwit),
        "taproot" | "p2tr" | "bech32m" => Ok(AddressType::Taproot),
        _ => Err(format!(
            "unknown address type '{s}'.  Choose from: legacy, p2sh, segwit, taproot"
        )),
    }
}

impl AddressType {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            AddressType::Legacy => "Legacy (P2PKH)",
            AddressType::P2sh => "Nested SegWit (P2SH)",
            AddressType::Segwit => "Native SegWit (P2WPKH)",
            AddressType::Taproot => "Taproot (P2TR)",
        }
    }

    /// Prefix hint for the address type (first characters).
    pub fn prefix_hint(self) -> &'static str {
        match self {
            AddressType::Legacy => "1",
            AddressType::P2sh => "3",
            AddressType::Segwit => "bc1q",
            AddressType::Taproot => "bc1p",
        }
    }
}

/// Validate that a prefix is syntactically possible for the chosen address type.
pub fn validate_prefix(prefix: &str, addr_type: AddressType) -> Result<(), String> {
    let hint = addr_type.prefix_hint();
    if !prefix.starts_with(hint) {
        return Err(format!(
            "Address type {} must start with '{}', but prefix is \"{prefix}\"",
            addr_type.label(),
            hint,
        ));
    }
    // For Legacy / P2SH the rest must be valid Base58 characters.
    if addr_type == AddressType::Legacy || addr_type == AddressType::P2sh {
        const BASE58: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        for c in prefix.chars() {
            if !BASE58.contains(c) {
                return Err(format!(
                    "Character '{c}' is not in the Base58 alphabet (no 0/O/I/l)."
                ));
            }
        }

        // ── Base58Check version-byte constraints ──────────────────
        // Due to Base58Check encoding, the second character of a P2SH
        // address (version 0x05) is *always* uppercase (A–R) or digit
        // (1–9) — never a lowercase letter.
        if prefix.len() >= 2 {
            let c2 = prefix.as_bytes()[1] as char;
            if addr_type == AddressType::P2sh && c2.is_lowercase() {
                let suggestion = make_second_upper(prefix);
                return Err(format!(
                    "P2SH address prefix cannot have a lowercase letter at position 2 \
                     (Base58Check encoding constraint).  Try '{suggestion}'."
                ));
            }
        }
    }
    // For SegWit / Taproot, every character after the "bc1q" / "bc1p" prefix
    // must be a valid Bech32 character (lowercase alphanumeric, excluding 1/b/i/o).
    if addr_type == AddressType::Segwit || addr_type == AddressType::Taproot {
        for c in prefix.chars() {
            let valid = c.is_ascii_lowercase() || c.is_ascii_digit();
            if !valid {
                return Err(format!(
                    "Bech32(m) addresses only allow lowercase letters and digits; got '{c}'."
                ));
            }
        }
    }
    Ok(())
}

/// Capitalise the second character of a prefix (e.g., "3qq" → "3Qq").
fn make_second_upper(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 && chars[1].is_lowercase() {
        chars[1] = chars[1].to_uppercase().next().unwrap_or(chars[1]);
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_type_parsing() {
        assert_eq!(parse_address_type("legacy").unwrap(), AddressType::Legacy);
        assert_eq!(parse_address_type("p2pkh").unwrap(), AddressType::Legacy);
        assert_eq!(parse_address_type("p2sh").unwrap(), AddressType::P2sh);
        assert_eq!(parse_address_type("segwit").unwrap(), AddressType::Segwit);
        assert_eq!(
            parse_address_type("native-segwit").unwrap(),
            AddressType::Segwit
        );
        assert_eq!(parse_address_type("bech32").unwrap(), AddressType::Segwit);
        assert_eq!(parse_address_type("taproot").unwrap(), AddressType::Taproot);
        assert_eq!(parse_address_type("p2tr").unwrap(), AddressType::Taproot);
        assert_eq!(parse_address_type("bech32m").unwrap(), AddressType::Taproot);
        assert!(parse_address_type("invalid").is_err());
    }

    #[test]
    fn test_address_type_label() {
        assert!(AddressType::Legacy.label().contains("Legacy"));
        assert!(AddressType::P2sh.label().contains("P2SH"));
        assert!(AddressType::Segwit.label().contains("SegWit"));
        assert!(AddressType::Taproot.label().contains("Taproot"));
    }

    #[test]
    fn test_address_type_prefix_hint() {
        assert_eq!(AddressType::Legacy.prefix_hint(), "1");
        assert_eq!(AddressType::P2sh.prefix_hint(), "3");
        assert_eq!(AddressType::Segwit.prefix_hint(), "bc1q");
        assert_eq!(AddressType::Taproot.prefix_hint(), "bc1p");
    }

    #[test]
    fn test_validate_legacy_prefix() {
        assert!(validate_prefix("1Bit", AddressType::Legacy).is_ok());
        assert!(validate_prefix("1Love", AddressType::Legacy).is_ok());
        // wrong first char
        assert!(validate_prefix("2Bit", AddressType::Legacy).is_err());
        assert!(validate_prefix("3Bit", AddressType::Legacy).is_err());
    }

    #[test]
    fn test_validate_p2sh_prefix() {
        assert!(validate_prefix("3Bit", AddressType::P2sh).is_ok());
        // lowercase second char → should error
        assert!(validate_prefix("3qBit", AddressType::P2sh).is_err());
        // uppercase second char → OK
        assert!(validate_prefix("3QBit", AddressType::P2sh).is_ok());
    }

    #[test]
    fn test_validate_segwit_prefix() {
        assert!(validate_prefix("bc1qbit", AddressType::Segwit).is_ok());
        assert!(validate_prefix("bc1qabc", AddressType::Segwit).is_ok());
        // uppercase not allowed in bech32
        assert!(validate_prefix("bc1Qbit", AddressType::Segwit).is_err());
        // wrong prefix
        assert!(validate_prefix("1Bit", AddressType::Segwit).is_err());
    }

    #[test]
    fn test_validate_taproot_prefix() {
        assert!(validate_prefix("bc1pbit", AddressType::Taproot).is_ok());
        assert!(validate_prefix("1Bit", AddressType::Taproot).is_err());
    }

    #[test]
    fn test_make_second_upper() {
        assert_eq!(make_second_upper("3qq"), "3Qq");
        assert_eq!(make_second_upper("3QQ"), "3QQ"); // already upper
        assert_eq!(make_second_upper("1"), "1"); // too short
        assert_eq!(make_second_upper(""), "");
    }
}
