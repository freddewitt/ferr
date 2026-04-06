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
6. [Renaming Templates](#renaming-templates)

---

## Installation

```sh
git clone https://github.com/freddewitt/ferr
cd ferr
cargo build --release
# Binary: target/release/ferr
```

---

## Global Usage

```
ferr <COMMAND> [OPTIONS]
ferr --version
ferr --help
ferr <COMMAND> --help
```

---

## Environment Variables

| Variable | Effect |
|----------|--------|
| `NO_COLOR` | Disables ANSI colors in terminal output |
| `FERR_DATA_DIR` | Custom path for the SQLite session database (default: `~/.local/share/ferr/`) |
| `RUST_LOG` | Log level for internal diagnostics (e.g. `RUST_LOG=warn`) |

---

## Commands

---

### `copy`

Secure file transfer with on-the-fly hashing, hash verification, and optional redundancy.

The copy verifies integrity automatically: source is hashed during read, destination is re-read and compared after write. If hashes differ, the transfer is retried once automatically.

```sh
ferr copy <SRC> <DEST> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `SRC` | path | required | Source file or directory |
| `DEST` | path | required | Primary destination directory |
| `--dest2 <PATH>` | path | — | Second destination (parallel mirror copy) |
| `--dest3 <PATH>` | path | — | Third destination |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Hash algorithm used for verification |
| `--par2 <PCT>` | 1–100 | — | Generate PAR2 redundancy files (N% overhead) |
| `--resume` | flag | false | Resume a partial copy (skips already verified files) |
| `--camera` | flag | false | Enable camera format detection (BRAW, R3D, ARRI, Sony, Canon, ProRes) |
| `--rename <TEMPLATE>` | string | — | Dynamic filename renaming (see [Renaming Templates](#renaming-templates)) |
| `--eject` | flag | false | Auto-eject source volume after successful transfer |
| `--dedup` | flag | false | Skip files already present in session history (by hash) |
| `--profile <NAME>` | string | — | Load settings from a saved profile |
| `--no-preserve-meta` | flag | false | Do not copy timestamps and extended attributes (xattrs) |
| `--no-notify` | flag | false | Disable system desktop notifications |
| `--pdf` | flag | false | Generate a PDF report in the destination after copy |
| `--report` | flag | false | Save a `ferr-manifest.json` in the destination after copy |
| `--dry-run` | flag | false | Simulate the job without writing any files |
| `--quiet` | flag | false | Suppress progress bars (suitable for scripting/CI) |
| `--progress-format <FORMAT>` | `human` \| `machine` | `human` | Output format: human-readable or machine-parseable lines |

#### Examples

```sh
# Copy a single file
ferr copy /Volumes/MAG/clip_001.braw /mnt/backup

# Mirror a card to two drives with SHA-256 and 5% PAR2
ferr copy /Volumes/MAG /Volumes/RAID_1 --dest2 /Volumes/RAID_2 --hash sha256 --par2 5 --camera

# Resume an interrupted transfer
ferr copy /Volumes/MAG /Volumes/RAID_1 --resume

# Dry run to preview without writing
ferr copy /Volumes/MAG /Volumes/RAID_1 --dry-run

# Copy and save a JSON manifest + PDF report
ferr copy /Volumes/MAG /Volumes/RAID_1 --report --pdf
```

---

### `verify`

Verifies the integrity of a destination against a source or a manifest file.

```sh
ferr verify <SRC_OR_MANIFEST> <DEST> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `SRC_OR_MANIFEST` | path | Source file/directory, or a `ferr-manifest.json` file |
| `DEST` | path | Destination file or directory to verify |
| `--quiet` | flag | Suppress output, rely on exit code only |

If `SRC_OR_MANIFEST` has a `.json` extension, ferr verifies against the manifest (hash-by-hash). Otherwise, it compares source and destination file-by-file.

#### Examples

```sh
# Verify a single file against its source
ferr verify source/clip_001.braw backup/clip_001.braw

# Verify a full directory
ferr verify /Volumes/MAG /Volumes/RAID_1

# Verify against a saved manifest
ferr verify /Volumes/RAID_1/_ferr_logs_20260405_120000/ferr-manifest.json /Volumes/RAID_1
```

---

### `repair`

Attempts to repair corrupted files using PAR2 parity data.

```sh
ferr repair <PAR2_INDEX> <TARGET>
```

| Argument | Type | Description |
|----------|------|-------------|
| `PAR2_INDEX` | path | Path to the `.par2` index file (e.g. `_par2/clip.par2`) |
| `TARGET` | path | File or directory to repair |

#### Example

```sh
ferr repair cible/_par2/cible.par2 cible/fw_rx2.par
```

---

### `scan`

Detects silent data corruption (bit rot) by re-hashing a destination and comparing against a manifest.

```sh
ferr scan <DEST> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `DEST` | path | Directory to scan |
| `--manifest <PATH>` | path | Manifest to compare against (auto-detected if omitted) |
| `--since <DATE>` | RFC3339 | Skip files with a recorded date after this threshold |
| `--quiet` | flag | Suppress output |

#### Example

```sh
ferr scan /Volumes/Archive --since 2025-01-01T00:00:00Z
```

---

### `watch`

Monitors a mount point and automatically triggers a copy when a volume is detected.

```sh
ferr watch <MOUNT_POINT> --dest <PATH> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `MOUNT_POINT` | path | required | Directory to monitor (e.g. `/Volumes`) |
| `--dest <PATH>` | path (repeatable) | required | One or more destination paths |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Hash algorithm |
| `--par2 <PCT>` | 1–100 | — | PAR2 redundancy percentage |
| `--camera` | flag | false | Enable camera format detection |
| `--profile <NAME>` | string | — | Load settings from a saved profile |
| `--delay <SECS>` | integer | `3` | Seconds to wait after volume detection before starting |
| `--eject` | flag | false | Auto-eject volume after transfer |
| `--quiet` | flag | false | Suppress output |

#### Example

```sh
ferr watch /Volumes --dest /mnt/backup --dest /mnt/backup2 --camera --eject --par2 10
```

---

### `export`

Converts a `ferr-manifest.json` to post-production formats.

```sh
ferr export <MANIFEST> --format <FORMAT> --output <PATH>
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `MANIFEST` | path | required | Path to a `ferr-manifest.json` |
| `--format <FORMAT>` | `ale` \| `csv` | `csv` | Output format |
| `--output <PATH>` | path | required | Output file path |

#### Example

```sh
ferr export ferr-manifest.json --format ale --output daily_log.ale
ferr export ferr-manifest.json --format csv --output day01.csv
```

---

### `report`

Generates a PDF report from a manifest.

```sh
ferr report <MANIFEST> [--output <PATH>]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `MANIFEST` | path | Path to a `ferr-manifest.json` |
| `--output <PATH>` | path | Output PDF path (defaults to same directory as manifest) |

#### Example

```sh
ferr report ferr-manifest.json --output DIT_Report_Day01.pdf
```

---

### `profile`

Saves and manages reusable copy configurations.

#### `profile save`

```sh
ferr profile save <NAME> --dest <PATH> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `NAME` | string | Profile name |
| `--dest <PATH>` | path (repeatable) | One or more destination paths |
| `--hash <ALGO>` | `xxhash` \| `sha256` | Hash algorithm |
| `--par2 <PCT>` | 1–100 | PAR2 redundancy |
| `--camera` | flag | Enable camera mode |
| `--eject` | flag | Auto-eject after transfer |

#### `profile list`

```sh
ferr profile list
```

#### `profile show`

```sh
ferr profile show <NAME>
```

#### `profile delete`

```sh
ferr profile delete <NAME>
```

#### Example

```sh
ferr profile save dit_standard --dest /mnt/r1 --dest /mnt/r2 --hash sha256 --par2 10 --camera
ferr copy /Volumes/MAG /mnt/r1 --profile dit_standard
```

---

### `history`

Browses the local SQLite session history.

#### `history list`

```sh
ferr history list [OPTIONS]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--limit <N>` | `20` | Number of recent sessions to show |
| `--since <DATE>` | — | Filter sessions after this RFC3339 date |

#### `history show`

```sh
ferr history show <ID>
```

Shows the full detail of a session by its numeric ID.

#### `history find`

```sh
ferr history find <HASH_OR_NAME>
```

Searches all sessions for a specific file by its hash or filename.

---

### `cert`

Manages **Portable Integrity Certificates** (`.ferrcert`) — self-contained signed files for cross-machine verification.

#### `cert create`

```sh
ferr cert create <SRC> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `SRC` | path | required | Source directory to certify |
| `--output <PATH>` | path | — | Output `.ferrcert` file path |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Hash algorithm |
| `--quiet` | flag | false | Suppress output |

#### `cert verify`

```sh
ferr cert verify <CERT> <DEST> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `CERT` | path | Path to the `.ferrcert` file |
| `DEST` | path | Destination directory to verify against |
| `--quiet` | flag | Suppress output |

#### Example

```sh
ferr cert create /Volumes/MAG --output day01.ferrcert --hash sha256
ferr cert verify day01.ferrcert /Volumes/RAID_1
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success — all files OK |
| `1` | Missing files detected |
| `2` | Corrupted files detected (hash mismatch) |
| `3` | Both missing and corrupted files |

---

## Renaming Templates

Used with `--rename` in `copy` and `watch`. Variables are replaced at copy time using camera metadata.

| Variable | Description |
|----------|-------------|
| `{camera}` | Camera identifier (e.g. `A`, `B`) |
| `{date}` | Recording date (`YYYYMMDD`) |
| `{clip}` | Original clip name |
| `{ext}` | File extension |

#### Example

```sh
ferr copy /Volumes/MAG /mnt/backup --camera --rename "{date}_{camera}_{clip}.{ext}"
```
