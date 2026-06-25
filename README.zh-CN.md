# vanitygen

[![CI](https://github.com/gamepunk/vanitygen/actions/workflows/ci.yml/badge.svg)](https://github.com/gamepunk/vanitygen/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/vanitygen.svg)](https://crates.io/crates/vanitygen)
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
# 通过 crates.io 安装（推荐）：
cargo install vanitygen

# 或从源码构建：
cargo install --git https://github.com/gamepunk/vanitygen.git

# 然后运行：
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

### `vanitygen` / `vanitygen search` — 搜索虚荣地址

**默认命令。** 查找与模式匹配的地址（支持前缀、后缀、子串或正则）。

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
| `-p, --match-prefix` | (默认) | 前缀匹配 |
| `-s, --suffix` | 关闭 | 后缀匹配 |
| `-a, --anywhere` | 关闭 | 子串匹配（地址任意位置） |
| `-r, --regex` | 关闭 | 正则表达式匹配 |
| `--input-file FILE` | — | 从文件读取模式（每行一个） |
| `-o, --output-file FILE` | — | 将结果追加写入文件 |

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

>> Same-key addresses
  Legacy (P2PKH): 1Pizza5RqXnupPvn9KbK8cLTBPcVB8zFhE
  Nested SegWit (P2SH): 3Ji9hUqTq15rQd...
  Native SegWit (P2WPKH): bc1qpy7y0...
  Taproot (P2TR): bc1pxv50f...
```

大小写不敏感搜索（更快，地址字母大小写随机）：
```bash
vanitygen 1bit -i -T 8
```

非压缩公钥（仅 Legacy 类型）：
```bash
vanitygen 1Pizza -u -T 8
```

BIP39 助记词模式（更慢，但输出 24 词助记词）：
```bash
vanitygen 1Pizza -m
```

其他地址类型：
```bash
vanitygen bc1qbit -t segwit
vanitygen 3Pizza -t p2sh
vanitygen bc1pbit -t taproot
```

**匹配模式：**

默认按**前缀**匹配（地址以该模式开头）。使用以下标志改变匹配行为：

```bash
# 后缀模式 — 地址以 "pizza" 结尾
vanitygen pizza -s -t segwit

# 后缀 + 大小写不敏感
vanitygen pizza -s -i -t segwit

# 子串模式 — 地址包含 "ninja"
vanitygen ninja -a -t segwit

# 正则模式 — 支持 regex crate 的全部语法
vanitygen '^1[A-Z]{3}.*[0-9]{2}$' -r -t legacy

# 正则 + 类似后缀匹配
vanitygen 'pizza$' -r -t segwit
```

**批量模式（输入/输出文件）：**

创建一个模式文件，每行一个模式（空行和 `#` 注释行会被忽略）：

```bash
vanitygen --input-file patterns.txt -o results.txt -t segwit
```

输入文件格式：
```
# 我的模式
1Bitcoin
ninja
pizza
```

结果以结构化格式追加到输出文件：
```
---
pattern: 1Bitcoin
mode: Prefix
address: 1BitcoinXXXXXXXXXXXXXX
wif: Lxxxxxxxxxxxxxxxxxxxxxxx
attempts: 12345
elapsed: 10.23s
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

从单个私钥派生出全部 4 种地址格式。

```
$ vanitygen address Kz6K83ge1AeeDi7fvE7kxGkyYws47sucXUZZwMXVTFG9q7u4ey12
>> Addresses from private key
  network: Mainnet
  compressed: true

>> Derived addresses
  Legacy (P2PKH): 1Ninja2TuXomkKakWbMzb9VBG8aj5krLbF
  Nested SegWit (P2SH): 37nx7BGgtq28QbRfMAdHYg2zsjmGBiVtuQ
  Native SegWit (P2WPKH): bc1qaeqa7easxmtfzr2jrpaqex9t6nudj0887p8cdq
  Taproot (P2TR): bc1pm3xcsp9ys2y6f2elt0yqzycrdkssdv4xhznjudqn2r07k2ympvusdnazap
```

---

### `vanitygen mnemonic` — 生成随机 BIP39 钱包

生成 24 词 BIP39 助记词（256 位），并为全部 4 条标准 BIP32 路径派生地址。

```
$ vanitygen mnemonic
>> BIP39 Mnemonic (24 words, 256-bit)
  abandon ability able about above absent abstract ...

>> Legacy (P2PKH)
  path: m/44'/0'/0'/0/0
  WIF: L25wxdxzuhRbAZ5ScDf...
  Legacy (P2PKH): 1Htr2MgUzhxRLuzAb3HB6HxgEoBe2Cswmq
  Nested SegWit (P2SH): 3MAJvD3BF4EvvponEDZQckPKc5TaY9oCSz
  Native SegWit (P2WPKH): bc1qh9gzvddydxcx7w2wh28sypt7xj0xlgcy9pngc6
  Taproot (P2TR): bc1p84pg4cl2zxda5k6lydugnh2umdsc8e5035303ss3f0pqvxmcdmqsc6r43y
```

⚠ 请手写抄下这 24 个单词，离线保存。任何人拥有此助记词即可窃取你的资金。

---

### `vanitygen benchmark` — 性能测试

通过随机密钥派生全部 4 种地址类型来测量吞吐量。

```
$ vanitygen benchmark
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
vanitygen v0.4.0
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
