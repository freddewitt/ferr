<div align="center">
  <h1>ferr</h1>
  <p><strong>Secure, Byte-for-Byte CLI File Copy Tool for DITs & Power Users</strong></p>

  [![CI](https://github.com/freddewitt/ferr/actions/workflows/ci.yml/badge.svg)](https://github.com/freddewitt/ferr/actions/workflows/ci.yml)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
  [![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
</div>

<br>

> [!WARNING]
> **Disclaimer: Personal Project & Vibe Coding**  
> I created this tool strictly for my own personal use. I am not a professional software developer, and this entire project is the result of "vibe coding" (built through exploration and AI assistance).  
> The code is provided **"as is"**, without any warranties, safety guarantees, or liability. Use it at your own risk.

---

**ferr** is a high-performance command-line utility designed for **absolute data integrity**. Whether you're a Digital Imaging Technician (DIT) managing 8K cinema footage or a power user moving critical backups, `ferr` ensures every byte is accounted for through cryptographic verification and hardware-accelerated redundancy.

## 🚀 Why use `ferr`?

Traditional copy tools can fail silently or leave you wondering if your data is actually safe. `ferr` bridges the gap between a simple `cp` and complex industrial workflows:

- 🛡️ **Zero-Trust Copying**: Every file is hashed on-the-fly (`XXH64` or `SHA-256`).
- ⚡ **Multi-Destination**: Copy from one source to up to 3 destinations simultaneously.
- 🏗️ **Self-Healing Data**: Native `par2` support—if a bit flips on your drive, `ferr` can repair it.
- 💾 **Smart History**: Powered by a local SQLite database to prevent duplicate copies.
- 📽️ **Cinema-Ready**: Automatic detection of BRAW, R3D, ARRI, Sony, and more.

---

## 🏎️ Quick Start

### Installation
```bash
# Requires Rust 1.75+
git clone https://github.com/freddewitt/ferr
cd ferr
cargo install --path ferr-cli
```

### Basic Usage
```bash
# Secure copy to one destination (default XXH64)
ferr copy /Volumes/SOURCE /Volumes/DEST

# Mirror copy to two SSDs with 5% PAR2 redundancy
ferr copy /Volumes/CARD /mnt/ssd1 --dest2 /mnt/ssd2 --par2 5
```

---

## 🎖️ Feature Highlights

### 1. Portable Integrity Certificates (`ferr cert`)
Portable certificates allow you to vouch for the integrity of a folder without sharing your entire session database. Useful for shipping drives to post-production labs or clients.

- **Create**: `ferr cert create /MyData --output verification.ferrcert`
- **Verify**: `ferr cert verify verification.ferrcert /ReceivedData`
- *Perfect for shipping drives to post-production labs or clients.*

### 2. DIT Automation & Renaming
Detect camera formats and rename clips based on metadata dynamically:
```bash
ferr copy /Volumes/A001 /RAID --camera --rename "{date}_{camera}_{reel}_{clip}{ext}" --eject
```

### 3. "Watch" Mode
Set it and forget it. `ferr` monitors mount points and kicks off copies as soon as a card is inserted:
```bash
ferr watch /Volumes --dest /mnt/backups --profile onset
```

---

## 🏗️ System Architecture

`ferr` is built as a modular Rust workspace for maximum speed and reliability:

| Component | Responsibility |
| :--- | :--- |
| **`ferr-core`** | Orchestration, dry-runs, and job logic. |
| **`ferr-cert`** | Portable PEM-encoded integrity certificates. |
| **`ferr-par2`** | Native PAR2 verification and repair engine. |
| **`ferr-hash`** | High-performance streaming hashing (xxHash/SHA2). |
| **`ferr-session`** | SQLite-backed transfer history and deduplication. |
| **`ferr-report`** | JSON manifest architecture and PDF generation. |

---

## 📖 Documentation & Reference

For a complete breakdown of every command, diagnostic flag, and real-world DIT automation scenario, please refer to the:

👉 **[ferr CLI Reference Guide](CLI.md)**

```bash
# Get context-specific help at any time
ferr copy --help
ferr cert --help
```

---

## 📜 License
Licensed under the **MIT License**. See [LICENSE](LICENSE) for details.
