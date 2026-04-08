# ferr — CLI Reference

**ferr** is a command-line utility for copying files and verifying their integrity. It is designed for workflows where data verification is important, such as in digital media production or backup management.

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

Copies files and directories while calculating and verifying hashes to assist with transfer integrity.

The tool hashes the source files during reading and re-hashes the destination files after writing to compare them. If a mismatch is detected, it will attempt to retry the transfer once. **A `.ferrcert` certificate is automatically generated for every copy operation.**

```sh
ferr copy <SRC> <DEST> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `SRC` | path | required | Source directory (directories only) |
| `DEST` | path | required | Primary destination directory |
| `--dest2 <PATH>` | path | — | Second destination for parallel copies |
| `--dest3 <PATH>` | path | — | Third destination |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Hash algorithm for verification |
| `--par2 <PCT>` | 1–100 | — | Generate PAR2 redundancy files (N% overhead) |
| `--resume` | flag | false | Skip files that appear to have been already verified |
| `--camera` | flag | false | Enable detection for common camera formats |
| `--rename <TEMPLATE>` | string | — | Dynamic renaming (see [Renaming Templates](#renaming-templates)) |
| `--eject` | flag | false | Attempt to eject the source volume after transfer |
| `--dedup` | flag | false | Check session history to skip previously copied files |
| `--profile <NAME>` | string | — | Use settings from a saved profile |
| `--no-preserve-meta` | flag | false | Do not attempt to preserve timestamps and attributes |
| `--no-notify` | flag | false | Disable system notifications |
| `--pdf` | flag | false | Generate a PDF report in the destination |
| `--report` | flag | false | Save a `ferr-manifest.json` report |
| `--dry-run` | flag | false | Show what would be done without modifying files |
| `--quiet` | flag | false | Reduce terminal output |
| `--progress-format <FORMAT>` | `human` \| `machine` | `human` | Output format for progress info |

#### Examples

```sh
# Basic folder copy
ferr copy /Volumes/MAG /mnt/backup

# Copy with SHA-256 and PAR2 redundancy
ferr copy /Volumes/MAG /Volumes/RAID_1 --dest2 /Volumes/RAID_2 --hash sha256 --par2 5 --camera

# Resume a previous copy
ferr copy /Volumes/MAG /Volumes/RAID_1 --resume
```

---

### `verify`

Checks the integrity of a destination by comparing it against a source, a `.ferrcert`, or a JSON manifest.

```sh
ferr verify <SRC_OR_MANIFEST> <DEST> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `SRC_OR_MANIFEST` | path | Source path, `.ferrcert` file, or `.json` manifest |
| `DEST` | path | Path to verify |
| `--quiet` | flag | Minimize terminal output |

The command will automatically detect the type of input based on the file extension.

#### Examples

```sh
# Verify two directories
ferr verify /Volumes/MAG /Volumes/RAID_1

# Verify against a certificate
ferr verify /Volumes/RAID_1/MAG.ferrcert /Volumes/RAID_1
```

---

### `repair`

Uses PAR2 data to attempt to fix corrupted files.

```sh
ferr repair <PAR2_INDEX> <TARGET>
```

| Argument | Type | Description |
|----------|------|-------------|
| `PAR2_INDEX` | path | Path to the `.par2` index file |
| `TARGET` | path | File or directory to attempt to repair |

---

### `scan`

Scans a directory for changes or corruption by re-calculating hashes and comparing them to a stored record.

```sh
ferr scan <DEST> [OPTIONS]
```

| Argument / Flag | Type | Description |
|-----------------|------|-------------|
| `DEST` | path | Directory to scan |
| `--manifest <PATH>` | path | Record to compare against (auto-detected if omitted) |
| `--since <DATE>` | RFC3339 | Ignore files modified after this date |
| `--quiet` | flag | Minimize output |

---

### `watch`

Monitors a path for new volume mounts to trigger copy operations.

```sh
ferr watch <MOUNT_POINT> --dest <PATH> [OPTIONS]
```

| Argument / Flag | Type | Default | Description |
|-----------------|------|---------|-------------|
| `MOUNT_POINT` | path | required | Path to monitor (e.g. `/Volumes`) |
| `--dest <PATH>` | path | required | Destination(s) for the copies |
| `--delay <SECS>` | integer | `3` | Seconds to wait after detection before starting |

---

### `export`

Converts a `ferr-manifest.json` report into other formats like ALE or CSV.

```sh
ferr export <MANIFEST> --format <FORMAT> --output <PATH>
```

---

### `report`

Generates a PDF report from a manifest file.

```sh
ferr report <MANIFEST> [--output <PATH>]
```

---

### `profile`

Manages configuration profiles for `copy` and `watch` commands.

```sh
ferr profile save <NAME> --dest <PATH> [OPTIONS]
ferr profile list
ferr profile show <NAME>
ferr profile delete <NAME>
```

---

### `history`

Provides access to the local record of previous copy sessions.

```sh
ferr history list [--limit <N>] [--since <DATE>]
ferr history show <ID>
ferr history find <HASH_OR_NAME>
```

---

### `cert`

Manages `.ferrcert` integrity certificates. For more details on the certificate format, see [ferrcert.md](ferrcert.md).

#### `cert create`

```sh
ferr cert create <SRC> [--output <PATH>] [--hash <ALGO>] [--quiet]
```

#### `cert verify`

```sh
ferr cert verify <CERT> <DEST> [--quiet]
```

#### `cert show`

```sh
ferr cert show <CERT_OR_DIR> [--quiet]
```

#### `cert check`

```sh
ferr cert check <CERT_OR_DIR>
```

Checks the internal integrity of the certificate file itself.

---

## Exit Codes

| Code | Description |
|------|-------------|
| `0` | Success / Verification passed |
| `1` | One or more files are missing |
| `2` | Hash mismatch / Data corruption / Invalid certificate hash |
| `3` | Both missing and corrupted files detected |
| `4` | Minor issues found (e.g. empty directories missing) |

---

## Renaming Templates

Used with the `--rename` flag. Variables are replaced by metadata if available.

| Variable | Description |
|----------|-------------|
| `{camera}` | Camera identifier |
| `{date}` | Recording date (YYYYMMDD) |
| `{clip}` | Original clip name |
| `{ext}` | File extension |
