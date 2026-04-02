# Changelog

All notable changes to the **Ferr** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-04-02

### Added
- **Desktop Graphical Interface** (`ferr-app`): Introduced a complete, multilingual (English/French) desktop GUI, built using Tauri v2. It provides a sleek, user-friendly interface for managing copies, verifying drives, consulting history, and configuring settings, seamlessly communicating with the native Rust engine.
- **Native PAR2 Engine** (`ferr-par2`): Native verification and repair using the `rust-par2` library, eliminating external dependencies for these operations. Implemented `Par2View` to handle directory structures and in-place repairs via high-performance symlinking.
- **Ferr Certificate** (`ferr-cert`, `ferr-cli`): New subcommand `ferr cert` to generate and verify portable, JSON-based cryptographic certificates for folders and files, allowing third-party integrity checks without a local database.
- **Workspace Expansion**: Integration of the `ferr-cert` and `ferr-app` crates into the core workspace.
- **Architecture**: Unified the error reporting and progress handling between subprocess-based generation and native verification.

### Changed
- Refactored `ferr-par2` for better maintainability and performance during repair operations.

## [0.1.0] - 2026-03-29

### Added
- **Core Copy Engine** (`ferr-core`, `ferr-transfer`): Secure asynchronous copy featuring disk space verification, atomic file transfers, and preservation of extended attributes (xattrs) and modification dates.
- **On-the-fly Hashing** (`ferr-hash`): Computes `xxhash64` and `sha256` signatures simultaneously during the copy stream without requiring a second disk pass.
- **Hardware Redundancy** (`ferr-par2`): Generates PAR2 parity data via a `par2cmdline` subprocess to protect against potential data decay.
- **Watch Mode**: Native monitoring (`notify-rust`) of directories and mount points, triggering automatic copies upon volume insertion. Includes specific automation options for DIT workflows.
- **Camera Formats Detection** (`ferr-camera`): Optional DIT feature that recognizes BRAW, R3D, ARRI MXF, Sony XOCN, Canon XF, and ProRes formats for dynamic file renaming via templates (e.g., `{camera}`, `{date}`, `{clip}`).
- **Local SQLite Database** (`ferr-session`): Tracks every copied file to ensure auditability and provide a deduplication feature (skips copies if the exact hash was securely transferred in the past).
- **Reports and Manifests** (`ferr-report`, `ferr-pdf`): Automatically generates a cryptographic JSON manifest after each session, as well as a formatted PDF report.
- **Post-Production Exports**: Allows converting generated manifests into Avid (ALE) formats and standard CSV files.
- **Verification and Bit-rot Scanner** (`ferr-verify`): Cold storage utilities designed to verify the integrity of a disk against a past manifest.
- **User Profiles**: Manage long backup parameters with `ferr profile` commands.
- **Native Notifications**: End-of-process alerts integrated directly into the OS notification system.

### Initial Release
- First stable technical milestone. Full implementation of a multi-crate Cargo architecture.
- 100% of integration tests are passing.
