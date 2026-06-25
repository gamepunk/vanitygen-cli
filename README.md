# vanity

[![CI](https://github.com/gamepunk/vanity/actions/workflows/ci.yml/badge.svg)](https://github.com/gamepunk/vanity/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/vanitygen.svg)](https://crates.io/crates/vanitygen)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[中文版](README.zh-CN.md)

**Bitcoin vanity address generator — find your perfect address.**

Generate custom Bitcoin addresses with a chosen prefix
(e.g. `1Bit`, `1Pizza`, `bc1qninja`).  Supports all 4 address formats.

All cryptographic primitives are delegated to the audited
[`rust-bitcoin`](https://github.com/rust-bitcoin/rust-bitcoin) crate
(backed by `libsecp256k1` — the same C library Bitcoin Core uses).

---

## Install

### Prerequisites

- **Rust 1.70+** — install via [rustup](https://rustup.rs) (all platforms)
- Nothing else — no OpenSSL, no system libraries

### Build

```bash
cargo build --release
./target/release/vanity 1Bit
```

Or install globally:

```bash
cargo install --git <repo-url>
vanity 1Bit
```

### Platform notes

| Platform | Build | Bark notifications |
|----------|-------|-------------------|
| macOS    | ✅ native | ✅ uses system `curl` |
| Linux    | ✅ native | ✅ uses `curl` |
| Windows  | ✅ native | ❌ not supported (no `curl`) |

Bark is optional and purely cosmetic — the tool works fully on all platforms
without it.

---

## Commands

### `vanity` / `vanity search` — Search for a vanity address

**Default command.**  Find an address whose string starts with a given prefix.

```
vanity 1Bit
vanity search 1Bit
```

**Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `-t, --address-type TYPE` | `legacy` | Address type |
| `-i, --case-insensitive` | off | Match any letter case (faster) |
| `-m, --mnemonic` | off | Use BIP39+BIP32 derivation (outputs seed phrase) |
| `-u, --uncompressed` | off | Uncompressed public key (Legacy only) |
| `-T, --threads N` | all cores | Worker threads |
| `-q, --quiet` | off | Suppress progress output |
| `--bark KEY` | — | Bark API key for iOS push notification |

**Examples:**

Search Legacy, normal mode (fast, outputs WIF):
```
$ vanity 1Pizza -T 8
>> Self-test passed
>> Searching
  prefix: 1Pizza
  type: Legacy (P2PKH)
  threads: 8

>> Found vanity address
  Address: 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE
  WIF: L2VH7b3xpLkv1jN8bPNdw73tK8fH...   ← import this into your wallet
  attempts: 10,317
  elapsed: 11.2s

>> Same-key addresses
  P2PKH: 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE
  P2SH: 3Ji9hUqTq15rQd...
  P2WPKH: bc1qpy7y0...
  P2TR: bc1pxv50f...
```

Search with mnemonic (slower, outputs 24-word seed phrase):
```
$ vanity 1Pizza -m
>> Searching
  prefix: 1Pizza
  type: Legacy (P2PKH)
  threads: 8
  source: BIP39+BIP32

>> Found vanity address
  Address: 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE
  WIF: L2VH7b3xpLkv1jN8bPNdw73tK8fH...

>> BIP39 Mnemonic
  abandon ability able about above absent...

  derivation path: m/44'/0'/0'/0/0

>> Wallet addresses (index 0)
  P2PKH: 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE  (path: m/44'/0'/0'/0/0)
  P2SH: 3Ji9hUqTq15rQd...                      (path: m/49'/0'/0'/0/0)
  P2WPKH: bc1qpy7y0...                          (path: m/84'/0'/0'/0/0)
  P2TR: bc1pxv50f...                             (path: m/86'/0'/0'/0/0)
```

Search other address types:
```
vanity bc1qbit -t segwit
vanity 3Pizza -t p2sh
vanity bc1pbit -t taproot
```

---

### `vanity verify` — Validate a WIF private key

Parse a WIF and show all four derived addresses.

```
$ vanity verify Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12
>> WIF Verification
  network: Mainnet
  compressed: true

>> Derived addresses
  P2PKH: 1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF
  P2SH: 37nx7BGgtq28QbRfMAdHYg2zsjmGBiVtuQ
  P2WPKH: bc1qaeqa7easxmtfzr2jrpaqex9t6nudj0887p8cdq
  P2TR: bc1pm3xcsp9ys2y6f2elt0yqzycrdkssdv4xhznjudqn2r07k2ympvusdnazap
```

---

### `vanity address` — Derive all address types from a key

Shows all four address formats from a single private key.

```
$ vanity address Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12
>> Addresses from private key
  network: Mainnet
  compressed: true

>> Derived addresses
  P2PKH: 1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF
  P2SH: 37nx7BGgtq28QbRfMAdHYg2zsjmGBiVtuQ
  P2WPKH: bc1qaeqa7easxmtfzr2jrpaqex9t6nudj0887p8cdq
  P2TR: bc1pm3xcsp9ys2y6f2elt0yqzycrdkssdv4xhznjudqn2r07k2ympvusdnazap
```

---

### `vanity mnemonic` — Generate a random BIP39 wallet

Creates a 24-word BIP39 mnemonic (256-bit) and derives addresses
for all 4 standard BIP32 paths at index 0.

```
$ vanity mnemonic
>> BIP39 Mnemonic (24 words, 256-bit)
  abandon ability able about above absent abstract ...

>> Legacy (P2PKH)
  path: m/44'/0'/0'/0/0
  WIF: L25wxdxzuhRbAZ5ScDf...
  P2PKH: 1Htr2MgUzhxRLuzAb3HB6HxgEoBe2Cswmq
  P2SH: 3MAJvD3BF4EvvponEDZQckPKc5TaY9oCSz
  P2WPKH: bc1qh9gzvddydxcx7w2wh28sypt7xj0xlgcy9pngc6
  P2TR: bc1p84pg4cl2zxda5k6lydugnh2umdsc8e5035303ss3f0pqvxmcdmqsc6r43y
```

⚠ Write down these 24 words.  Keep them offline.  Anyone with this phrase
can steal your funds.

---

### `vanity benchmark` — Measure performance

Derives all 4 address types from random keys to measure throughput.

```
$ vanity benchmark
>> Benchmark
  threads: 8
  iterations: 400000 (50000 per thread)

>> Results
  elapsed: 3.315s
  keys derived: 400000
  speed: 0.12 Mkeys/s
  threads: 8
  per thread: 15.08 kkeys/s
```

---

## Address Types

| Type | CLI name | Prefix | Encoding | BIP standard |
|------|----------|--------|----------|-------------|
| Legacy (P2PKH) | `legacy` | `1…` | Base58Check | BIP44 |
| P2SH-SegWit | `p2sh` | `3…` | Base58Check | BIP49 |
| Native SegWit | `segwit` | `bc1q…` | Bech32 | BIP84 |
| Taproot | `taproot` | `bc1p…` | Bech32m | BIP86 |

---

## Performance

Timings for **normal mode** on an M-series Mac (8 threads, ~1 Mkeys/s).

| Effective chars | Avg tries | Normal mode | `--mnemonic` mode |
|:---:|---:|---:|---:|
| 2 | 3.3×10³ | < 0.1s | ~1s |
| 3 | 1.9×10⁵ | 0.2s | ~50s |
| 4 | 1.1×10⁷ | 11s | ~50 min |
| 5 | 6.5×10⁸ | 11 min | impractical |
| 6+ | 3.8×10¹⁰ | ~10 h | impractical |

Use `--mnemonic` for short prefixes (2–3 chars).  For longer prefixes use
normal mode and import the WIF directly.

---

## Notes

**Base58Check encoding:** The second character of a P2SH address (`3…`) is
always uppercase (A–R) or a digit (1–9) — never lowercase.  For Legacy
addresses (`1…`) it's lowercase only ~2% of the time.  The tool warns
you if you try an impossible prefix.

### Push notification (Bark)

Receive an iOS push notification when a match is found.

**Setup:** Get your free API key from the [Bark app](https://bark.day.app/).

```bash
# Via CLI flag:
vanity 1Pizza --bark YOUR_KEY_HERE

# Or via config file (recommended):
#   ~/.config/vanity/config.toml
#   bark_key = "YOUR_KEY_HERE"
```

CLI flag overrides config file.

Uses system `curl` — zero deps.  Not available on Windows (no `curl`).

---

## Configuration file

`vanity` loads settings from `~/.config/vanity/config.toml` (XDG standard).

```toml
# ~/.config/vanity/config.toml

# Default thread count (overridable by -T / --threads)
threads = 8

# Bark API key for iOS push notification
bark_key = "YOUR_KEY_HERE"

# Default address type (legacy, p2sh, segwit, taproot)
# address_type = "legacy"
```

CLI flags always override config file values.

---

## Dependencies

```
vanity v0.2.0
├── bip39       — BIP39 mnemonic generation
├── bitcoin     — Bitcoin address / key types
├── bs58        — Base58Check encoding (hot path)
├── clap        — CLI argument parsing
├── rand        — Cryptographic random numbers
├── ripemd      — RIPEMD-160 hashing (hot path)
└── sha2        — SHA-256 hashing (hot path)
```

---

## License

MIT
