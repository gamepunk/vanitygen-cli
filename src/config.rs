//! Configuration file support.
//!
//! Looks for `~/.config/vanitygen/config.toml` (XDG standard).
//! CLI flags always win over config-file values.

use std::path::PathBuf;

use serde::Deserialize;

/// Top-level config structure.
///
/// All fields are optional — anything not set in the file falls through
/// to the CLI default.
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Default thread count (overridable by `-T` / `--threads`).
    pub threads: Option<usize>,
    /// Bark API key for iOS push notifications.
    pub bark_key: Option<String>,
    /// Default address type: legacy, p2sh, segwit, taproot.
    pub address_type: Option<String>,

    /// Enable BIP39+BIP32 derivation by default.
    pub mnemonic: Option<bool>,
    /// Case-insensitive matching by default.
    pub case_insensitive: Option<bool>,
    /// Use uncompressed public key (Legacy only).
    pub uncompressed: Option<bool>,
    /// Default match mode: "prefix", "suffix", "anywhere", "regex".
    pub match_mode: Option<String>,
    /// BIP39 word count: 12, 15, 18, 21, or 24.
    pub words: Option<usize>,
    /// Suppress progress output by default.
    pub quiet: Option<bool>,
    /// Default output file path.
    pub output_file: Option<String>,
}

impl Config {
    /// Load config from the first available location.
    pub fn load() -> Self {
        let paths = candidate_paths();
        for path in &paths {
            if path.exists() {
                let raw = match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                match toml::from_str(&raw) {
                    Ok(cfg) => {
                        eprintln!("[config] loaded {}", path.display());
                        return cfg;
                    }
                    Err(e) => {
                        eprintln!("[config] parse error in {}: {e}", path.display());
                    }
                }
            }
        }
        // No valid config found → create default one.
        if let Some(path) = paths.first() {
            Self::create_default(path);
        }
        Config::default()
    }

    /// Write a commented default config file if it doesn't exist.
    fn create_default(path: &std::path::Path) {
        if path.exists() {
            return;
        }
        let content = r#"# vanitygen configuration
# See https://github.com/gamepunk/vanitygen for details
#
# All values are optional — CLI flags override these defaults.

# Number of worker threads (default: all logical cores)
# threads = 8

# Default address type: legacy, p2sh, segwit, taproot
# address_type = "legacy"

# Case-insensitive matching (faster)
# case_insensitive = true

# Use BIP39+BIP32 derivation (slower, outputs seed phrase)
# mnemonic = false

# Uncompressed public key (Legacy only)
# uncompressed = false

# Match mode: "prefix", "suffix", "anywhere", "regex"
# match_mode = "prefix"

# BIP39 word count: 12, 15, 18, 21, 24 (only with mnemonic = true)
# words = 24

# Suppress progress output
# quiet = false

# Default output file (results appended)
# output_file = "results.txt"

# Bark API key for iOS push notifications
# bark_key = "YOUR_KEY_HERE"
"#;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(path, content) {
            Ok(_) => eprintln!("[config] created default {}", path.display()),
            Err(e) => eprintln!("[config] failed to create {}: {e}", path.display()),
        }
    }
}

/// Return config file paths in priority order.
fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. XDG standard: $XDG_CONFIG_HOME/vanitygen/config.toml
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    paths.push(xdg.join("vanitygen").join("config.toml"));

    paths
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
threads = 4
bark_key = "test-key"
address_type = "segwit"
mnemonic = true
case_insensitive = true
uncompressed = false
match_mode = "anywhere"
words = 12
quiet = true
output_file = "out.txt"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.threads, Some(4));
        assert_eq!(cfg.bark_key.as_deref(), Some("test-key"));
        assert_eq!(cfg.address_type.as_deref(), Some("segwit"));
        assert_eq!(cfg.mnemonic, Some(true));
        assert_eq!(cfg.case_insensitive, Some(true));
        assert_eq!(cfg.uncompressed, Some(false));
        assert_eq!(cfg.match_mode.as_deref(), Some("anywhere"));
        assert_eq!(cfg.words, Some(12));
        assert_eq!(cfg.quiet, Some(true));
        assert_eq!(cfg.output_file.as_deref(), Some("out.txt"));
    }

    #[test]
    fn test_parse_empty_config() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.threads.is_none());
        assert!(cfg.bark_key.is_none());
        assert!(cfg.address_type.is_none());
        assert!(cfg.mnemonic.is_none());
        assert!(cfg.case_insensitive.is_none());
        assert!(cfg.match_mode.is_none());
    }

    #[test]
    fn test_parse_partial_config() {
        let toml_str = "threads = 16\nquiet = true\n";
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.threads, Some(16));
        assert!(cfg.bark_key.is_none());
        assert_eq!(cfg.quiet, Some(true));
    }

    #[test]
    fn test_candidate_paths_contains_xdg() {
        let paths = candidate_paths();
        assert!(!paths.is_empty());
        assert!(paths[0].to_string_lossy().contains("vanitygen"));
        assert!(paths[0].to_string_lossy().contains("config.toml"));
    }
}
