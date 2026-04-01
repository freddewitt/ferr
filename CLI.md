# ferr — CLI Reference

**ferr** is a specialized tool for Digital Imaging Technicians (DITs) and power users who require absolute data integrity and workflow automation.

---

## Table of Contents

1. [Installation](#installation)
2. [Global Usage](#global-usage)
3. [Environment Variables](#environment-variables)
4. [Commands](#commands)
   - [copy](#copy)
   - [verify](#verify)
   - [repair](#repair)
   - [scan](#scan)
   - [watch](#watch)
   - [export](#export)
   - [report](#report)
   - [profile](#profile)
   - [history](#history)
   - [cert](#cert)
5. [Exit Codes](#exit-codes)
6. [Data Formats](#data-formats)
7. [Renaming Templates](#renaming-templates)
8. [Session History](#session-history)

---

## Installation

To build the project from source, ensure you have Rust 1.75+ installed:

```sh
git clone https://github.com/freddewitt/ferr
cd ferr
cargo build --release
# The binary is located at target/release/ferr
```

---

## Global Usage

```
ferr <COMMAND> [OPTIONS]
```

| Global Option | Description |
|---------------|-------------|
| `--version`   | Displays the current version of ferr |
| `--help`      | Displays detailed help for any command |

---

## Environment Variables

| Variable | Effect |
|----------|-------|
| `NO_COLOR` | Disables ANSI colors in terminal output |
| `FERR_DATA_DIR` | Custom path for the SQLite session database (Default: `~/.local/share/ferr/` on macOS/Linux) |

---

## Commands

### `copy`

Executes a secure file transfer with on-the-fly hashing and optional redundancy.

```sh
ferr copy <SRC> <DEST> [OPTIONS]
```

#### Arguments
- `SRC`: Source directory (e.g., camera card mount point)
- `DEST`: Primary destination directory

#### Options
| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--dest2 <PATH>` | Path | — | Second destination for parallel mirror copy |
| `--dest3 <PATH>` | Path | — | Third destination |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Cryptographic hash algorithm |
| `--par2 <PCT>` | 1–100 | — | Generate PAR2 redundancy files (N% overhead) |
| `--resume` | flag | false | Resume a partial copy (skips verified files) |
| `--camera` | flag | false | Enable heuristic camera format detection (BRAW, R3D, ARRI, etc.) |
| `--rename <TEMPLATE>` | string | — | Dynamic renaming pattern (see [Renaming](#renaming-templates)) |
| `--eject` | flag | false | Auto-eject source volume after successful transfer |
| `--dedup` | flag | false | Skip files if their hash is already in the history |
| `--profile <NAME>` | string | — | Load settings from a saved profile |
| `--no-preserve-meta` | flag | false | Do not copy timestamps and extended attributes |
| `--no-notify` | flag | false | Disable system desktop notifications |
| `--no-pdf` | flag | false | Skip PDF report generation |
| `--dry-run` | flag | false | Simulate the job without writing any files |
| `--quiet` | flag | false | Hide progress bars for scripting/CI |

#### Usage Scenarios
- **DIT Mirroring**: Copy a 1TB RED card to two separate RAID drives with SHA-256 verification and 5% recovery data:
  ```sh
  ferr copy /Volumes/MAG_A /Volumes/RAID_1 --dest2 /Volumes/RAID_2 --hash sha256 --par2 5 --camera
  ```
- **Resume Transfer**: If a drive was accidentally unplugged, simply rerun the command with `--resume` to skip already copied files.

---

### `verify`

Verifies the integrity of a destination against a source or a manifest.

```sh
ferr verify <SRC_OR_MANIFEST> <DEST> [--quiet]
```

#### Scenarios
- **Cold Storage Audit**: Verify a drive against its 1-year-old manifest:
  ```sh
  ferr verify /backup/day01/ferr-manifest.json /Volumes/OLD_DRIVE
  ```

---

### `cert`

Generates and manages **Portable Integrity Certificates** (`.ferrcert`). These are self-contained, signed PEM-like files used for cross-machine verification.

#### `cert create`
```sh
ferr cert create <SRC> --output my_transfer.ferrcert
```

#### `cert verify`
```sh
ferr cert verify my_transfer.ferrcert /mnt/received_data
```

---

### `watch`

Monitors a mount point (like `/Volumes`) and automatically starts copying as soon as a volume is detected.

```sh
ferr watch /Volumes --dest /mnt/backup --camera --eject
```

---

### `scan`

Detects "Bit Rot" (silent data corruption) by re-hashing a drive and comparing it to its original manifest.

```sh
# Scan everything and report mismatches
ferr scan /Volumes/MyStorage

# Scan only files modified after a specific date
ferr scan /Volumes/MyStorage --since 2025-01-01T00:00:00Z
```

---

### `export` & `report`

- **Export**: Converts a session manifest to post-production formats (`ALE` for Avid or `CSV`).
  ```sh
  ferr export ferr-manifest.json --format ale --output daily_log.ale
  ```
- **Report**: Generates a professional PDF report with file-by-file verification status.
  ```sh
  ferr report ferr-manifest.json --output DIT_Report_Day01.pdf
  ```

---

## Session History

Every successful transfer is recorded in a local SQLite database. This allows for:
- **`history list`**: See recent jobs.
- **`history find <HASH_OR_NAME>`**: Instantly locate where a specific clip was backed up across multiple drives.

---

*ferr v0.2.0 — Secure Data Management*
