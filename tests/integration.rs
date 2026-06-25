//! Integration tests for the vanitygen binary.
//!
//! These tests invoke the compiled binary as a subprocess and verify
//! its output, exit codes, and error messages.

use std::process::Command;

/// Path to the compiled binary.
fn vanitygen_bin() -> String {
    let mut path = std::env::current_dir().unwrap_or_default();
    // In CI the working dir is the project root; in `cargo test` the
    // working dir is also the project root.  The binary lives under
    // target/debug/ or target/release/.
    if cfg!(debug_assertions) {
        path.push("target/debug/vanitygen");
    } else {
        path.push("target/release/vanitygen");
    }
    path.to_string_lossy().to_string()
}

#[test]
fn test_help_exit_code() {
    let output = Command::new(vanitygen_bin())
        .arg("--help")
        .output()
        .expect("failed to run vanitygen");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("vanitygen"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("benchmark"));
    assert!(stdout.contains("mnemonic"));
}

#[test]
fn test_search_help() {
    let output = Command::new(vanitygen_bin())
        .args(["search", "--help"])
        .output()
        .expect("failed to run vanitygen search --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--suffix"));
    assert!(stdout.contains("--anywhere"));
    assert!(stdout.contains("--regex"));
    assert!(stdout.contains("--input-file"));
    assert!(stdout.contains("--output-file"));
    assert!(stdout.contains("--words"));
}

#[test]
fn test_version() {
    let output = Command::new(vanitygen_bin())
        .arg("--version")
        .output()
        .expect("failed to run vanitygen --version");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0.4.0"));
}

#[test]
fn test_verify_known_wif() {
    let wif = "Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12";
    let output = Command::new(vanitygen_bin())
        .args(["verify", wif])
        .output()
        .expect("failed to run vanitygen verify");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show the derived addresses with new labels.
    assert!(stdout.contains("Legacy (P2PKH)"));
    assert!(stdout.contains("Nested SegWit (P2SH)"));
    assert!(stdout.contains("Native SegWit (P2WPKH)"));
    assert!(stdout.contains("Taproot (P2TR)"));
    // Should contain the known address.
    assert!(stdout.contains("1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF"));
}

#[test]
fn test_verify_invalid_wif() {
    let output = Command::new(vanitygen_bin())
        .args(["verify", "not-a-wif"])
        .output()
        .expect("failed to run vanitygen verify");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("错误") || stderr.contains("invalid"));
}

#[test]
fn test_address_known_wif() {
    let wif = "Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12";
    let output = Command::new(vanitygen_bin())
        .args(["address", wif])
        .output()
        .expect("failed to run vanitygen address");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Legacy (P2PKH)"));
    assert!(stdout.contains("1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF"));
}

#[test]
fn test_benchmark_runs() {
    let output = Command::new(vanitygen_bin())
        .args(["benchmark"])
        .output()
        .expect("failed to run vanitygen benchmark");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let all_output = format!("{}{}", stdout, stderr);
    assert!(
        output.status.success(),
        "exit code: {}\nstdout: {}\nstderr: {}",
        output.status,
        stdout,
        stderr
    );
    assert!(
        all_output.contains("keys derived")
            || all_output.contains("Results")
            || all_output.contains("speed"),
        "benchmark output missing expected content:\n{}",
        all_output
    );
}

#[test]
fn test_mnemonic_help() {
    let output = Command::new(vanitygen_bin())
        .args(["mnemonic", "--help"])
        .output()
        .expect("failed to run vanitygen mnemonic --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--words"));
}

#[test]
fn test_search_bad_prefix_rejected() {
    // A prefix that doesn't match the address type should fail.
    let output = Command::new(vanitygen_bin())
        .args(["search", "1BadPrefix", "-t", "segwit"])
        .output()
        .expect("failed to run vanitygen search");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("错误") || stderr.contains("must start with"));
}

#[test]
fn test_search_quiet_mode_fast() {
    // Run a very short search (short prefix) in quiet mode.
    // This should find a result quickly without progress output.
    let output = Command::new(vanitygen_bin())
        .args(["search", "1A", "-t", "legacy", "-T", "2", "-q"])
        .output()
        .expect("failed to run vanitygen search");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let all_output = format!("{}{}", stdout, stderr);
    // Debug output on failure
    assert!(
        output.status.success(),
        "exit code: {}\nstdout: {}\nstderr: {}",
        output.status,
        stdout,
        stderr
    );
    assert!(
        stdout.contains("Address:") || all_output.contains("Address:"),
        "Missing 'Address:' in output:\n{}",
        all_output
    );
    assert!(
        stdout.contains("WIF:") || all_output.contains("WIF:"),
        "Missing 'WIF:' in output:\n{}",
        all_output
    );
    // attempts: and elapsed: are printed via style::kv() which goes to stderr
    assert!(
        stderr.contains("attempts:") || all_output.contains("attempts:"),
        "Missing 'attempts:' in output:\n{}",
        all_output
    );
}
