# Changelog

All notable changes to the **ferr** project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-04-20

### Added
- **New Application Identity**: Replaced placeholder icons with a custom high-resolution logo across all platforms (macOS, Windows, Linux).
- **Icon Generation Pipeline**: Integrated a full suite of optimized icons (ICO, ICNS, and multi-scale PNGs) into the build process.

### Changed
- Updated all workspace crates to version `0.5.0`.

## [0.4.0] - 2026-04-08

### Added
- **Integrity Certificates v1.0** (`.ferrcert`): Complete redesign of the portable certificate format.
    - **Self-Signing**: Certificates are now signed with an internal SHA-256 hash to prevent manual tampering.
    - **Event History**: Tracks the certificate's lifecycle (creation, copies, verifications) with hostname and timestamp.
    - **Hierarchy (Tree)**: Includes a full recursive map of the certified directory, including file sizes and hashes.
    - **Empty Directory Tracking**: Improved tracking of empty folders to ensure destination parity.
- **Improved CLI Commands**:
    - `ferr cert show`: Detailed view of a certificate's contents and history.
    - `ferr cert check`: Standalone integrity verification for certificate files.
- **Mandatory Certification**: `ferr copy` now automatically generates a `.ferrcert` for every job and verifies the destination against it.

### Changed
- **Directory-Only Copying**: `ferr copy` now strictly requires directory paths as source to ensure consistent integrity tracking.
- **Verification Logic**: Commands like `verify` and `scan` now prioritize `.ferrcert` files over legacy JSON manifests.
- **GUI Refinement**:
    - Restricted source selection to folders in the copy tab.
    - Added "Certified" status awareness to the source folder picker.
    - Simplified the health tab to focus on the new certificate workflow.

### Internal
- Complete rewrite of the `ferr-cert` crate.
- Deep integration of certification logic into the `ferr-core` copy engine.
- Updated all workspace crates to version `0.4.0`.


## [0.3.0] - 2026-04-06

### Added
- **Certificate Management via GUI** (`ferr-app`): Added support for creating and verifying legacy and `.ferrcert` Integrity Certificates in the Health tab.
- **Internationalization (i18n)**: Translated the CLI, logging system, error messages, and notifications from French to English.
- **Log Organization**: Reports (JSON manifests and PDF logs) are now grouped in timestamped subdirectories (`_ferr_logs_YYYYMMDD_HHMMSS`) at the destination root. Added auto-discovery for these manifests in `verify` and `scan` commands.
- **Documentation Updates**: Added compilation instructions to the README. Updated `CLI.md` with parameter tables and additional examples.

### Changed
- Updated all crates to version `0.3.0`.

## [0.2.0] - 2026-04-02

### Added
- **Desktop Graphical Interface** (`ferr-app`): Added a desktop GUI built with Tauri v2 (English/French support) for managing copy jobs and viewing history.
- **Native PAR2 Engine** (`ferr-par2`): Replaced external dependencies for PAR2 verification and repair with the `rust-par2` library. Added `Par2View` for directory structure handling.
- **Integrity Certificates**: Added the `cert` subcommand to generate and verify portable JSON certificates.
- **Workspace Expansion**: Added `ferr-cert` and `ferr-app` to the project workspace.
- **Architecture**: Standardized error reporting and progress handling across components.

### Changed
- Refactored `ferr-par2` internals.

## [0.1.0] - 2026-03-29

### Added
- **Core Copy Engine**: Implementation of asynchronous file copying with basic verification and metadata preservation.
- **Hashing**: Added support for `xxhash64` and `sha256` hashing during the copy process.
- **Redundancy**: Integration of `par2cmdline` for parity data generation.
- **Watch Mode**: Monitoring of mount points to trigger copy operations.
- **Camera Format Detection**: Recognition of several camera file formats for metadata-based renaming.
- **Session History**: Uses SQLite to track copy jobs and support hash-based deduplication.
- **Reports**: Generation of JSON manifests and PDF reports.
- **Exports**: Support for exporting manifests to ALE and CSV formats.
- **Verification Tools**: Added `verify` and `scan` commands for checking data integrity.
- **Profiles**: Support for saving and loading job configurations.
- **Notifications**: Basic system alerts for job completion.

### Initial Release
- Initial implementation of the multi-crate architecture.
- Core features for DIT workflows and basic data verification.
