<div align="center">
  <h1>ferr</h1>
  <p><strong>Secure, Byte-for-Byte CLI File Copy Tool</strong></p>

  [![CI](https://github.com/freddewitt/ferr/actions/workflows/ci.yml/badge.svg)](https://github.com/freddewitt/ferr/actions/workflows/ci.yml)
</div>

<br>

> [!WARNING]
> **Disclaimer: Personal Project & Vibe Coding**  
> I created this tool strictly for my own personal use. I am not a professional software developer, and this entire project is the result of "vibe coding" (built through exploration and AI assistance).  
> The code is provided **"as is"**, without any warranties, safety guarantees, or liability. Use it at your own risk.

<br>

**ferr** is a secure command-line file copy tool designed to guarantee absolute data integrity when transferring critical files across drives. While built as a general-purpose secure copy software, it also features advanced workflow automation options tailored specifically for **DITs** (Digital Imaging Technicians).

## ✨ Key Features

- **Parallel Transfers**: Copy from any source volume to one or multiple destinations simultaneously.
- **Cryptographic Hashing**: Byte-for-byte hash verification (`XXH64` or `SHA-256`) calculated on the fly.
- **Data Redundancy**: Native `par2` verification and repair. (Generation still uses `par2cmdline` for maximum reliability).
- **Portable Certificates**: Generate standalone JSON-based integrity certificates (`ferr cert`) for any file or folder.
- **Session Manifests**: Cryptographically signed JSON manifests generated at the end of every transfer.
- **Metadata Preservation**: Seamlessly carries over file timestamps and extended attributes (macOS `xattr`).
- **DIT Automation**: Detects cinema camera formats (BRAW, R3D, ARRI, Sony, Canon, ProRes) and offers dynamic `{date}_{camera}_{clip}` renaming templates.
- **Post-Production Ready**: One-click ALE (Avid) and CSV exports, complete with time-stamped PDF reports.
- **SQLite History**: Powerful local database to track all previous sessions and provide intelligent transfer deduplication.
- **Watch Mode**: Set and forget. Automatically triggers copies as soon as a new volume is inserted.

---

## 🚀 Installation & Prerequisites

**Prerequisites:**
- **Rust stable ≥ 1.75** (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **par2cmdline** *(Required only for PAR2 generation)*: `brew install par2`
- **Native Support**: Verification and repair are built-in (no external tools required).

**Install from source:**
```sh
git clone https://github.com/your-org/ferr
cd ferr
cargo install --path ferr-cli

# Verify installation
ferr --version
```

---

## 📖 Quick Reference Guide

### Copying Files

```sh
# Basic copy with XXH64 hashing and JSON manifest creation
ferr copy /Volumes/DriveA /mnt/backup

# Mirror copy to two destinations with SHA-256 and 10% PAR2 redundancy
ferr copy /Volumes/DriveA /mnt/ssd1 --dest2 /mnt/ssd2 --hash sha256 --par2 10

# DIT Mode: Detect camera format, rename files on the fly, and auto-eject volume
ferr copy /Volumes/CARD /mnt/raid \
    --camera \
    --rename "{date}_{camera}_{reel}_{clip}{ext}" \
    --eject

# Dry-run copy (simulates the entire process without writing anywhere)
ferr copy /Volumes/DriveA /mnt/backup --dry-run
```

### Verification & Bit-Rot Scanning

```sh
# Verify a destination folder against its source
ferr verify /Volumes/DriveA /mnt/backup

# Verify a backup using its previously generated JSON manifest
ferr verify /mnt/backup/ferr-manifest.json /mnt/backup

# Scan for bit-rot or data degradation on cold storage
ferr scan /mnt/backup

# Generate a portable integrity certificate for a folder
ferr cert create /Volumes/DriveA --output my_data.ferrcert

# Verify a folder against a received certificate
ferr cert verify my_data.ferrcert /mnt/receiver/DriveA
```

### Automation (Watch Mode)

Monitor a directory (like `/Volumes` on macOS) and automatically fire a dual-destination copy every time a new memory card is mounted.

```sh
ferr watch /Volumes --dest /mnt/ssd1 --dest /mnt/ssd2 --camera --eject
```

### Reporting & Export

```sh
# Export ALE for Avid Media Composer
ferr export ferr-manifest.json --format ale --output session.ale

# Generate an elegant PDF report
ferr report ferr-manifest.json --output report.pdf
```

### Session Profiles & History

```sh
# Save your exact configuration parameters as a reusable profile
ferr profile save backup_template --dest /mnt/ssd1 --dest /mnt/ssd2 --hash sha256 --par2 10

# Run a copy using your saved profile
ferr copy /Volumes/DriveA /ignored --profile backup_template

# Search through your SQLite transfer history
ferr history list
ferr history find A001_C001.braw
```

---

## 🏗 System Architecture

Under the hood, `ferr` uses a multi-crate Cargo workspace to isolate its logic into solid, testable components:

- **`ferr-core`**: The main orchestrator (handles validation, dry-runs, disk space logic).
- **`ferr-transfer`**: Manages atomic file copying to prevent partial writes.
- **`ferr-hash`**: High-performance streaming hashing.
- **`ferr-camera`**: Heuristic format detection and renaming engine.
- **`ferr-par2`**: Mixed-mode PAR2 engine (Native Verify/Repair, Subprocess Generation).
- **`ferr-cert`**: Engine for standalone portable integrity certificates.
- **`ferr-report`** / **`ferr-verify`**: JSON/ALE generation and cold-storage scanning tools.
- **`ferr-session`**: Bundled local SQLite `history.db` tracking deduplication.

---

## 📜 License

MIT — see [LICENSE](LICENSE) for more details.
