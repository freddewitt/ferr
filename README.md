# ferr

[![CI](https://github.com/freddewitt/ferr/actions/workflows/ci.yml/badge.svg)](https://github.com/freddewitt/ferr/actions/workflows/ci.yml)

Outil CLI de copie sécurisée de fichiers vidéo conçu pour les **DIT** (Digital Imaging Technician) et les professionnels de l'audiovisuel.

- Copie de carte mémoire vers une ou plusieurs destinations en parallèle
- Vérification hash (XxHash64 ou SHA-256) à chaque octet
- Redondance PAR2 via `par2cmdline`
- Détection de format caméra (BRAW, R3D, ARRI MXF, Sony XOCN, Canon XF, ProRes)
- Manifest JSON signé à chaque session
- Export ALE (Avid) et CSV pour post-production
- Rapport PDF horodaté
- Historique SQLite des sessions
- Mode watch : copie automatique à l'insertion d'un volume
- Profils de copie persistants
- NO_COLOR, `--quiet`, `--dry-run`

---

## Prérequis

| Dépendance | Requis | Installation |
|-----------|--------|-------------|
| Rust stable ≥ 1.75 | Oui | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| `par2cmdline` | Non (PAR2 désactivé si absent) | `brew install par2` / `apt install par2` / `winget install par2cmdline` |

---

## Installation

```sh
# Depuis les sources
git clone https://github.com/your-org/ferr
cd ferr
cargo install --path ferr-cli

# Vérifier
ferr --version
```

---

## Utilisation rapide

### Copie

```sh
# Copie de base (hash XxHash64, manifest JSON)
ferr copy /Volumes/A001 /mnt/backup

# Copie miroir vers deux destinations avec SHA-256 et PAR2 10 %
ferr copy /Volumes/A001 /mnt/ssd1 --dest2 /mnt/ssd2 --hash sha256 --par2 10

# Mode caméra + renommage + éjection automatique
ferr copy /Volumes/A001 /mnt/raid \
    --camera \
    --rename "{date}_{camera}_{reel}_{clip}{ext}" \
    --eject

# Simulation avant la vraie copie
ferr copy /Volumes/A001 /mnt/backup --dry-run

# Reprise d'une copie interrompue
ferr copy /Volumes/A001 /mnt/backup --resume

# Avec un profil sauvegardé
ferr copy /Volumes/A001 /ignored --profile onset
```

### Vérification et scan

```sh
# Vérifier depuis la source
ferr verify /Volumes/A001 /mnt/backup

# Vérifier depuis un manifest
ferr verify /mnt/backup/ferr-manifest.json /mnt/backup

# Détecter le bit rot (comparaison hash vs manifest)
ferr scan /mnt/backup
ferr scan /mnt/backup --since 2025-01-01T00:00:00Z  # incrémental
```

### Export et rapport

```sh
# Export ALE pour Avid Media Composer
ferr export ferr-manifest.json --format ale --output session.ale

# Export CSV
ferr export ferr-manifest.json --format csv --output session.csv

# Rapport PDF
ferr report ferr-manifest.json --output report.pdf
```

### Watch — copie automatique

```sh
# Surveiller /Volumes, copier vers deux SSD à chaque insertion de carte
ferr watch /Volumes --dest /mnt/ssd1 --dest /mnt/ssd2 --camera --eject
```

### Profils

```sh
ferr profile save onset --dest /mnt/ssd1 --dest /mnt/ssd2 --hash sha256 --par2 10 --camera --eject
ferr profile list
ferr profile show onset
ferr profile delete onset
```

### Historique

```sh
ferr history list
ferr history list --limit 50 --since 2025-01-01T00:00:00Z
ferr history show 42
ferr history find A001_C001.braw
```

---

## Architecture

```
ferr/
├── ferr-hash       # Hashing streaming — XxHash64 (xxhash-rust) et SHA-256 (sha2)
├── ferr-report     # Manifest JSON + export ALE/CSV
├── ferr-transfer   # Copie atomique (tmp → rename), métadonnées, xattrs (macOS)
├── ferr-verify     # Vérification de répertoires, manifests et scan bit rot
├── ferr-par2       # Génération/vérification/réparation PAR2 via subprocess par2cmdline
├── ferr-camera     # Détection de format caméra, scan de clips, templates de renommage
├── ferr-session    # Historique SQLite des sessions (rusqlite bundled)
├── ferr-notify     # Notifications système (notify-rust)
├── ferr-pdf        # Rapports PDF (printpdf)
├── ferr-core       # Orchestration : run_copy, run_watch, dry_run, profils, espaces disque
└── ferr-cli        # Binaire `ferr` — clap 4, indicatif, console
```

### Flux de copie (`ferr copy`)

```
[Source] ──► collect_files
               │
               ├─► check_space (libc::statvfs / GetDiskFreeSpaceExW)
               │
               ├─► [dédup] find_file_by_hash → session SQLite
               │
               ├─► copy_file (atomic write + fsync)
               │   └─► hash_file (XxHash64 / SHA-256)
               │   └─► copy_metadata (filetime + xattrs)
               │
               ├─► [PAR2] par2 create subprocess
               │
               ├─► save_manifest → ferr-manifest.json
               ├─► generate_report → report.pdf
               ├─► record_session → history.db
               └─► notify_done → notification système
```

---

## Variables d'environnement

| Variable | Valeur par défaut | Description |
|----------|-------------------|-------------|
| `NO_COLOR` | — | Désactive toutes les couleurs ANSI |
| `FERR_DATA_DIR` | `~/.local/share/ferr/` (Unix) / `%APPDATA%\ferr\` (Windows) | Répertoire de la base de sessions |
| `FERR_PAR2_STUB` | — | Force le mode stub PAR2 (CI sans par2cmdline) |

---

## Codes de sortie

| Code | Signification |
|------|--------------|
| 0 | Succès |
| 1 | Avertissement (fichiers manquants, bit rot détecté) |
| 2 | Erreur fatale |
| 3 | Irrécupérable (PAR2 insuffisant) |

---

## Développement

```sh
# Tests complets (unitaires + intégration)
cargo test

# Tests d'intégration PAR2 (nécessite par2cmdline)
cargo test -p ferr-par2 -- --include-ignored

# Build release
cargo build --release

# Linting
cargo clippy --workspace --all-targets -- -D warnings

# Format
cargo fmt --all
```

---

## Licence

MIT — voir [LICENSE](LICENSE).
