//! `verify` subcommand: parse a WIF, derive all four addresses, and display
//! them side-by-side for visual comparison.

use bitcoin::secp256k1::Secp256k1;
use bitcoin::Network;

use crate::address::derive_all;
use crate::error::Error;
use crate::wif;

/// Run the verify command.
pub fn run(wif_str: &str) -> Result<(), Error> {
    let info = wif::parse_wif(wif_str)?;
    let secp = Secp256k1::new();

    let set = derive_all(
        &secp,
        &info.private_key.inner,
        info.compressed,
        info.network,
    )?;

    let network_label = match info.network {
        Network::Bitcoin => "Mainnet",
        Network::Testnet => "Testnet",
        Network::Signet => "Signet",
        Network::Regtest => "Regtest",
        _ => "Unknown",
    };

    crate::style::header("WIF Verification");
    crate::style::kv("network", network_label);
    crate::style::kv("compressed", &info.compressed.to_string());
    println!();

    crate::style::header("Derived addresses");
    crate::style::result_line("Legacy (P2PKH)", &set.legacy.to_string());
    crate::style::result_line("Nested SegWit (P2SH)", &set.p2sh_segwit.to_string());
    crate::style::result_line("Native SegWit (P2WPKH)", &set.native_segwit.to_string());
    crate::style::result_line("Taproot (P2TR)", &set.taproot.to_string());
    println!();

    Ok(())
}
