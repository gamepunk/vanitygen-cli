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
./target/release/vanitygen 1Bit
```

或全局安装：

```bash
cargo install --git https://github.com/gamepunk/vanity.git
vanitygen 1Bit
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

### `vanitygen` / `vanity search` — 搜索虚荣地址

**默认命令。** 查找以指定前缀开头的地址。

```
vanitygen 1Bit
vanitygen search 1Bit
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
$ vanitygen 1Pizza -T 8
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

### `vanitygen verify` — 验证 WIF 私钥

```
$ vanitygen verify Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12
>> WIF Verification
  network: Mainnet
  compressed: true

>> Derived addresses
  Legacy (P2PKH): 1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF
  Nested SegWit (P2SH): 37nx7BGgtq28QbRfMAdHYg2zsjmGBiVtuQ
  Native SegWit (P2WPKH): bc1qaeqa7easxmtfzr2jrpaqex9t6nudj0887p8cdq
  Taproot (P2TR): bc1pm3xcsp9ys2y6f2elt0yqzycrdkssdv4xhznjudqn2r07k2ympvusdnazap
```

---

### `vanitygen address` — 查看私钥对应的全部地址

```
$ vanitygen address <WIF>
```

---

### `vanitygen mnemonic` — 生成随机 BIP39 钱包

生成 24 词助记词 + 4 条标准 BIP32 路径的地址。

---

### `vanitygen benchmark` — 性能测试

```
$ vanitygen benchmark
>> Benchmark
  threads: 8
  iterations: 400000
  speed: 0.12 Mkeys/s
```

---

## 地址类型

| 类型 | CLI 名称 | 前缀 | 编码 | BIP 标准 |
|------|----------|------|------|----------|
| Legacy (P2PKH) | `legacy` | `1…` | Base58Check | BIP44 |
| Nested SegWit (P2SH) | `p2sh` | `3…` | Base58Check | BIP49 |
| Native SegWit (P2WPKH) | `segwit` | `bc1q…` | Bech32 | BIP84 |
| Taproot (P2TR) | `taproot` | `bc1p…` | Bech32m | BIP86 |

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
vanitygen 1Pizza --bark YOUR_KEY_HERE

# 或通过配置文件（推荐）：
# ~/.config/vanitygen/config.toml
#   bark_key = "YOUR_KEY_HERE"
```

---

## 配置文件

`vanitygen` 从 `~/.config/vanitygen/config.toml`（XDG 标准）加载配置。

```toml
# ~/.config/vanitygen/config.toml

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
vanitygen v0.3.1
├── bip39       — BIP39 助记词生成
├── bitcoin     — 比特币地址/密钥类型
├── bs58        — Base58Check 编码（热路径）
├── clap        — CLI 参数解析
├── rand        — 密码学安全随机数
├── ripemd      — RIPEMD-160 哈希（热路径）
└── sha2        — SHA-256 哈希（热路径）
```

---

## 免责声明

**⚠ 使用风险自负。**

本工具在您的本地机器上生成比特币私钥。私钥和助记词以明文形式显示在
终端上。任何能够访问您屏幕、终端历史记录或剪贴板的人都可能窃取您的
资金。

- **资金到账后立即转走**，不要长期存放在虚荣地址中。
- **生成密钥后清除终端历史**（Unix 下执行 `history -c`，或重启终端）。
- **切勿**向任何人透露您的 WIF、助记词或私钥。
- **在离线（断网）机器上运行**以获得最大安全性。
- 作者**不承担**因使用本软件导致的任何资金损失或其他损害的责任。

使用本软件即表示您接受以上条款。

---

## 许可证

MIT
