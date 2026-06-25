# vanity

[![CI](https://github.com/gamepunk/vanity/actions/workflows/ci.yml/badge.svg)](https://github.com/gamepunk/vanity/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/badge/crates.io-v0.2.0-orange)](https://crates.io/crates/vanity)
[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

[English](README.md)

**比特币虚荣地址生成器 — 找到你的完美地址。**

生成指定前缀的比特币地址（如 `1Bit`、`1Pizza`、`bc1qninja`）。  
支持全部 4 种地址格式。

所有密码学原语均委托给经过审计的
[`rust-bitcoin`](https://github.com/rust-bitcoin/rust-bitcoin) 库
（底层是 `libsecp256k1` —— Bitcoin Core 使用的同一套 C 库）。

---

## 安装

### 前提

- **Rust 1.70+** — 通过 [rustup](https://rustup.rs) 安装（全平台）
- 不需要 OpenSSL 等任何系统库

### 构建

```bash
cargo build --release
./target/release/vanity 1Bit
```

或全局安装：

```bash
cargo install --git https://github.com/gamepunk/vanity.git
vanity 1Bit
```

### 平台支持

| 平台 | 构建 | Bark 通知 |
|----------|-------|-----------|
| macOS    | ✅ 原生 | ✅ 使用系统 `curl` |
| Linux    | ✅ 原生 | ✅ 使用 `curl` |
| Windows  | ✅ 原生 | ❌ 不支持（无 `curl`）|

Bark 为可选项，不影响核心功能。

---

## 命令

### `vanity` / `vanity search` — 搜索虚荣地址

**默认命令。** 查找以指定前缀开头的地址。

```
vanity 1Bit
vanity search 1Bit
```

**选项：**

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `-t, --address-type TYPE` | `legacy` | 地址类型 |
| `-i, --case-insensitive` | 关闭 | 大小写不敏感（更快） |
| `-m, --mnemonic` | 关闭 | BIP39+BIP32 模式（输出助记词） |
| `-u, --uncompressed` | 关闭 | 非压缩公钥（仅 Legacy） |
| `-T, --threads N` | 所有核心 | 工作线程数 |
| `-q, --quiet` | 关闭 | 静默模式 |
| `--bark KEY` | — | Bark iOS 推送密钥 |

**示例：**

```
$ vanity 1Pizza -T 8
>> Self-test passed
>> Searching
  prefix: 1Pizza
  type: Legacy (P2PKH)
  threads: 8

>> Found vanity address
  Address: 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE
  WIF: L2VH7b3xpLkv1jN8bPNdw73tK8fH...
  attempts: 10,317
  elapsed: 11.2s
```

---

### `vanity verify` — 验证 WIF 私钥

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

### `vanity address` — 查看私钥对应的全部地址

```
$ vanity address <WIF>
```

---

### `vanity mnemonic` — 生成随机 BIP39 钱包

生成 24 词助记词 + 4 条标准 BIP32 路径的地址。

---

### `vanity benchmark` — 性能测试

```
$ vanity benchmark
>> Benchmark
  threads: 8
  iterations: 400000
  speed: 0.12 Mkeys/s
```

---

## 地址类型

| 类型 | CLI 名称 | 前缀 | 编码 | BIP 标准 |
|------|----------|------|------|----------|
| 传统 (P2PKH) | `legacy` | `1…` | Base58Check | BIP44 |
| 嵌套 SegWit | `p2sh` | `3…` | Base58Check | BIP49 |
| 原生 SegWit | `segwit` | `bc1q…` | Bech32 | BIP84 |
| Taproot | `taproot` | `bc1p…` | Bech32m | BIP86 |

---

## 性能参考

M 系列 Mac（8 线程，~1 Mkeys/s）上的参考数据。

| 有效字符数 | 平均尝试 | 普通模式 | `--mnemonic` 模式 |
|:---:|---:|---:|---:|
| 2 | 3.3×10³ | < 0.1s | ~1s |
| 3 | 1.9×10⁵ | 0.2s | ~50s |
| 4 | 1.1×10⁷ | 11s | ~50 分钟 |
| 5+ | 6.5×10⁸ | 11 分钟+ | 不可行 |

---

## Bark 推送

在 iPhone 上接收搜索结果通知。

```bash
# 通过命令行参数：
vanity 1Pizza --bark YOUR_KEY_HERE

# 或通过配置文件（推荐）：
# ~/.config/vanity/config.toml
#   bark_key = "YOUR_KEY_HERE"
```

---

## 配置文件

`vanity` 从 `~/.config/vanity/config.toml`（XDG 标准）加载配置。

```toml
# ~/.config/vanity/config.toml

# 默认线程数
# threads = 8

# Bark API 密钥
# bark_key = "YOUR_KEY_HERE"

# 默认地址类型
# address_type = "legacy"
```

CLI 参数优先级高于配置文件。

---

## 依赖

```
vanity v0.2.0
├── bip39       — BIP39 助记词生成
├── bitcoin     — 比特币地址/密钥类型
├── bs58        — Base58Check 编码（热路径）
├── clap        — CLI 参数解析
├── rand        — 密码学安全随机数
├── ripemd      — RIPEMD-160 哈希（热路径）
└── sha2        — SHA-256 哈希（热路径）
```

---

## 许可证

MIT
