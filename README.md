<div align="center">
  <h1>ferr</h1>
  <p><strong>File copy tool with integrity verification for DITs and backups (GUI + CLI)</strong></p>
  <p><em>Ferr — completely available, sounds good in CLI (ferr copy src/ dest/), comes from the Latin <strong>ferre</strong> (to carry).</em></p>

  [![CI](https://github.com/freddewitt/ferr/actions/workflows/ci.yml/badge.svg)](https://github.com/freddewitt/ferr/actions/workflows/ci.yml)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
  [![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
</div>

<br>

> [!WARNING]
> **Disclaimer: Personal Project**  
> I created this tool for my own use. I am not a professional software developer, and this project was built with the help of AI to explore certain workflows.  
> The code is provided **"as is"**, without any warranties or guarantees. Use it at your own risk.

---

**ferr** is a utility designed to help ensure file integrity during transfers. It is intended for users who need to verify that files are copied correctly, such as Digital Imaging Technicians (DITs) or those managing backups. It uses cryptographic hashing to verify data and provides optional redundancy through PAR2. It is available as a Desktop GUI application and a Command-Line Interface.

## Features

`ferr` provides several tools to assist with data management and verification:

- **Hashing**: Files can be hashed during the copy process (`XXH64` or `SHA-256`) to check for transfer errors.
- **Multiple Destinations**: Supports copying from one source to up to 3 destinations.
- **PAR2 Support**: Can generate PAR2 files to assist with possible future data repair if corruption occurs.
- **Desktop GUI**: A cross-platform interface built with Tauri v2 and HTML/JS.
- **Transfer History**: Uses a local SQLite database to keep track of completed jobs.
- **File Detection**: Includes basic detection for various camera file formats.
- **Integrity Certificates**: Supports generating and verifying portable `.ferrcert` files to document folder state and tracking history.
- **Logging**: Generates timestamped subfolders (`_ferr_logs`) containing job reports.

---

## Quick Start

### Installation

#### 1. Command-Line Interface (CLI) Only
```bash
# Requires Rust 1.75+
git clone https://github.com/freddewitt/ferr
cd ferr
cargo install --path ferr-cli
```

#### 2. Desktop GUI (`ferr-app`)
To build the graphical application on macOS, you must compile the CLI as a sidecar:

```bash
# 1. Build the engine
cargo build --release -p ferr-cli

# 2. Copy to the Tauri binaries folder (Apple Silicon example)
cp target/release/ferr ferr-app/binaries/ferr-cli-aarch64-apple-darwin

# 3. Install Tauri v2 CLI
cargo install tauri-cli --version "^2.0.0"

# 4. Build the application
cd ferr-app
cargo tauri build
# The app bundle will be at: target/release/bundle/macos/ferr.app
```

### Basic Usage
```bash
# Copy a folder and verify integrity (defaults to XXH64)
ferr copy /Volumes/SOURCE /Volumes/DEST

# Copy to two destinations with 5% PAR2 redundancy
ferr copy /Volumes/CARD /mnt/ssd1 --dest2 /mnt/ssd2 --par2 5
```

---

## Usage Scenarios

### 1. Integrity Certificates (`ferr cert`)
You can generate portable certificates to document the state of a folder. These are useful for verifying data integrity across different systems.
For a detailed technical explanation of how they work, see [ferrcert.md](ferrcert.md).

- **Create**: `ferr cert create /MyData`
- **Verify**: `ferr cert verify /MyData/MyData.ferrcert /ReceivedData`

### 2. Rename on Copy
Basic support for renaming files based on metadata during the copy process:
```bash
ferr copy /Volumes/A001 /RAID --camera --rename "{date}_{camera}_{clip}{ext}"
```

### 3. Watch Mode
Monitors a path for new mounts and starts a copy automatically:
```bash
ferr watch /Volumes --dest /mnt/backups --profile onset
```

---

## Project Structure

`ferr` is organized into several modules:

| Component | Description |
| :--- | :--- |
| **`ferr-app`** | GUI front-end. |
| **`ferr-cli`** | CLI interface. |
| **`ferr-core`** | Core logic and job management. |
| **`ferr-cert`** | Logic for generating and verifying integrity certificates. |
| **`ferr-par2`** | Wrapper for PAR2 verification and repair. |
| **`ferr-hash`** | Hashing implementations. |
| **`ferr-session`** | Session management and SQLite storage. |
| **`ferr-report`** | Report generation (JSON and PDF). |

---

## Documentation

For more information on commands and options, see the:

👉 **[ferr CLI Reference Guide](CLI.md)**

---

## License
Licensed under the **MIT License**. See [LICENSE](LICENSE) for details.
