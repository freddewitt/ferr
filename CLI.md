# ferr — Référence CLI

**ferr** est un outil DIT (Digital Imaging Technician) pour la copie sécurisée de fichiers vidéo avec vérification hash, redondance PAR2 et journalisation.

---

## Table des matières

1. [Installation](#installation)
2. [Utilisation globale](#utilisation-globale)
3. [Variables d'environnement](#variables-denvironnement)
4. [Commandes](#commandes)
   - [copy](#copy)
   - [verify](#verify)
   - [repair](#repair)
   - [scan](#scan)
   - [watch](#watch)
   - [export](#export)
   - [report](#report)
   - [profile](#profile)
   - [history](#history)
5. [Codes de sortie](#codes-de-sortie)
6. [Formats de fichiers](#formats-de-fichiers)
7. [Modèles de renommage](#modèles-de-renommage)
8. [Profils](#profils)
9. [Historique des sessions](#historique-des-sessions)

---

## Installation

```sh
cargo build --release
# Le binaire se trouve dans target/release/ferr
```

---

## Utilisation globale

```
ferr <COMMANDE> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--version` | Affiche la version |
| `--help` | Affiche l'aide |

---

## Variables d'environnement

| Variable | Effet |
|----------|-------|
| `NO_COLOR` | Désactive toutes les couleurs dans la sortie (standard NO_COLOR) |
| `FERR_DATA_DIR` | Répertoire de la base de données de sessions (par défaut `~/.local/share/ferr/` sur Unix, `%APPDATA%\ferr\` sur Windows) |

---

## Commandes

### copy

Copie des fichiers avec vérification hash, génération optionnelle de PAR2, PDF et notification.

```
ferr copy <SRC> <DEST> [OPTIONS]
```

**Arguments positionnels**

| Argument | Description |
|----------|-------------|
| `SRC` | Répertoire source (carte mémoire, SSD, etc.) |
| `DEST` | Répertoire de destination principal |

**Options**

| Option | Type | Défaut | Description |
|--------|------|--------|-------------|
| `--dest2 <PATH>` | Chemin | — | Deuxième destination (copie miroir simultanée) |
| `--dest3 <PATH>` | Chemin | — | Troisième destination |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Algorithme de hachage |
| `--par2 <PCT>` | 1–100 | — | Génère des fichiers PAR2 avec N% de redondance |
| `--resume` | flag | false | Reprend une copie interrompue (saute les fichiers déjà copiés et vérifiés) |
| `--camera` | flag | false | Active la détection de format caméra (BRAW, R3D, ARRI MXF, Sony XOCN, Canon XF, ProRes) |
| `--rename <TEMPLATE>` | Chaîne | — | Modèle de renommage (voir [Modèles de renommage](#modèles-de-renommage)) |
| `--eject` | flag | false | Éjecte le volume source après une copie réussie |
| `--dedup` | flag | false | Saute les fichiers dont le hash est déjà présent dans l'historique |
| `--profile <NOM>` | Chaîne | — | Charge les options depuis un profil sauvegardé |
| `--no-preserve-meta` | flag | false | Ne préserve pas les timestamps et xattrs |
| `--no-notify` | flag | false | Désactive les notifications système |
| `--no-pdf` | flag | false | Ne génère pas de rapport PDF |
| `--dry-run` | flag | false | Simule la copie sans écrire aucun fichier |
| `--quiet` | flag | false | Mode silencieux (pas de barres de progression) |

**Mode dry-run**

En mode `--dry-run`, ferr affiche :
- Nombre de fichiers et taille totale
- Espace PAR2 estimé
- Durée estimée (base : 300 Mo/s)
- Disponibilité de l'espace disque pour chaque destination
- Nombre de clips détectés (si `--camera`)

Aucun fichier n'est écrit.

**Exemples**

```sh
# Copie simple avec hash xxhash
ferr copy /Volumes/A001 /backup/day01

# Copie miroir vers deux destinations avec SHA-256 et PAR2 10%
ferr copy /Volumes/A001 /backup/ssd1 --dest2 /backup/ssd2 --hash sha256 --par2 10

# Copie avec mode caméra, renommage et éjection automatique
ferr copy /Volumes/CARD /mnt/raid --camera --rename "{date}_{camera}_{reel}_{clip}{ext}" --eject

# Simulation avant copie réelle
ferr copy /Volumes/A001 /backup --dry-run

# Copie avec profil
ferr copy /Volumes/A001 /ignored --profile dailybackup
```

---

### verify

Vérifie l'intégrité d'une copie en comparant source ou manifest avec destination.

```
ferr verify <SRC_OR_MANIFEST> <DEST> [OPTIONS]
```

**Arguments positionnels**

| Argument | Description |
|----------|-------------|
| `SRC_OR_MANIFEST` | Répertoire source **ou** fichier manifest JSON (`.json`) |
| `DEST` | Répertoire de destination à vérifier |

**Options**

| Option | Description |
|--------|-------------|
| `--quiet` | Mode silencieux |

**Codes de sortie**

| Code | Signification |
|------|--------------|
| 0 | Tout OK |
| 1 | Fichiers manquants |
| 2 | Fichiers corrompus |
| 3 | Manquants et corrompus |

**Exemples**

```sh
# Vérifier depuis la source
ferr verify /Volumes/A001 /backup/day01

# Vérifier depuis un manifest
ferr verify /backup/day01/ferr-manifest.json /backup/day01
```

---

### repair

Tente de réparer les fichiers corrompus via les données PAR2.

```
ferr repair <MANIFEST> <DEST>
```

**Arguments positionnels**

| Argument | Description |
|----------|-------------|
| `MANIFEST` | Fichier manifest JSON de la session |
| `DEST` | Répertoire contenant les fichiers et les fichiers PAR2 |

**Codes de sortie**

| Code | Signification |
|------|--------------|
| 0 | Réparation réussie |
| 3 | Irréparable ou PAR2 non disponible |

**Exemple**

```sh
ferr repair /backup/day01/ferr-manifest.json /backup/day01
```

> **Note :** PAR2 est actuellement en mode stub. La compilation FFI complète est prévue en phase 3.

---

### scan

Détecte le bit rot sur une destination en recomputant les hashs et en les comparant au manifest.

```
ferr scan <DEST> [OPTIONS]
```

**Arguments positionnels**

| Argument | Description |
|----------|-------------|
| `DEST` | Répertoire à scanner |

**Options**

| Option | Type | Défaut | Description |
|--------|------|--------|-------------|
| `--manifest <PATH>` | Chemin | `<DEST>/ferr-manifest.json` | Manifest de référence |
| `--since <DATE>` | RFC 3339 | — | Ignore les fichiers non modifiés depuis cette date |
| `--quiet` | flag | false | Mode silencieux |

**Codes de sortie**

| Code | Signification |
|------|--------------|
| 0 | Aucun bit rot |
| 1 | Bit rot détecté |

**Exemple**

```sh
# Scan complet
ferr scan /backup/day01

# Scan incrémental (ignore les fichiers anciens)
ferr scan /backup/day01 --since 2025-01-01T00:00:00Z

# Scan avec manifest personnalisé
ferr scan /backup/day01 --manifest /reports/ferr-manifest.json
```

---

### watch

Surveille un point de montage et déclenche automatiquement une copie dès qu'un volume est détecté.

```
ferr watch <MOUNT_POINT> [OPTIONS]
```

**Arguments positionnels**

| Argument | Description |
|----------|-------------|
| `MOUNT_POINT` | Répertoire à surveiller (ex. `/Volumes` ou `D:\`) |

**Options**

| Option | Type | Défaut | Description |
|--------|------|--------|-------------|
| `--dest <PATH>` | Chemin (répétable) | — | Destination(s) de copie |
| `--hash <ALGO>` | `xxhash` \| `sha256` | `xxhash` | Algorithme de hachage |
| `--par2 <PCT>` | 1–100 | — | Redondance PAR2 |
| `--camera` | flag | false | Détection de format caméra |
| `--profile <NOM>` | Chaîne | — | Charge les options depuis un profil |
| `--delay <SEC>` | Entier | `3` | Délai (secondes) avant de démarrer la copie après détection |
| `--eject` | flag | false | Éjecte le volume après copie |
| `--quiet` | flag | false | Mode silencieux |

**Exemples**

```sh
# Mode DIT classique : surveille /Volumes, copie vers deux SSD
ferr watch /Volumes --dest /mnt/ssd1 --dest /mnt/ssd2 --camera --eject

# Avec profil
ferr watch /Volumes --profile onset
```

---

### export

Exporte un manifest ferr vers un format de post-production.

```
ferr export <MANIFEST> --format <FORMAT> --output <OUTPUT>
```

**Arguments**

| Argument | Description |
|----------|-------------|
| `MANIFEST` | Fichier manifest JSON |
| `--format` | `ale` (Avid Log Exchange) ou `csv` |
| `--output` | Fichier de sortie |

**Exemples**

```sh
# Export ALE pour Avid
ferr export ferr-manifest.json --format ale --output report.ale

# Export CSV
ferr export ferr-manifest.json --format csv --output report.csv
```

**Format ALE**

Le fichier ALE généré contient les colonnes standard : `Name`, `Size`, `Hash`, `HashAlgo`, `Date`, `Status`.

**Format CSV**

Colonnes : `path`, `size`, `hash_algo`, `hash`, `modified_at`, `status`, `par2_generated`.

---

### report

Génère un rapport PDF depuis un manifest ferr.

```
ferr report <MANIFEST> [--output <PATH>]
```

**Arguments**

| Argument | Type | Défaut | Description |
|----------|------|--------|-------------|
| `MANIFEST` | Chemin | — | Fichier manifest JSON |
| `--output <PATH>` | Chemin | `<manifest>.pdf` | Chemin du PDF généré |

**Contenu du rapport**

- En-tête : version ferr, date de génération, nom d'hôte
- Résumé : source, nombre de fichiers, taille totale, durée, statut global
- Tableau paginé des fichiers copiés (chemin, taille, hash tronqué, statut)
- Pied de page : hash du manifest (intégrité du rapport)

**Exemple**

```sh
ferr report /backup/day01/ferr-manifest.json
ferr report /backup/day01/ferr-manifest.json --output /reports/day01.pdf
```

---

### profile

Gère les profils de copie sauvegardés dans `~/.config/ferr/profiles/`.

```
ferr profile <SOUS-COMMANDE>
```

#### profile save

Sauvegarde un profil avec les options de copie souhaitées.

```
ferr profile save <NOM> [OPTIONS]
```

| Option | Type | Description |
|--------|------|-------------|
| `--dest <PATH>` | Chemin (répétable) | Destinations |
| `--hash <ALGO>` | `xxhash` \| `sha256` | Algorithme de hachage |
| `--par2 <PCT>` | 1–100 | Redondance PAR2 |
| `--camera` | flag | Mode caméra |
| `--eject` | flag | Éjection automatique |

```sh
ferr profile save onset --dest /mnt/ssd1 --dest /mnt/ssd2 --hash sha256 --par2 10 --camera --eject
```

#### profile list

Liste tous les profils sauvegardés.

```sh
ferr profile list
```

#### profile show

Affiche le contenu JSON d'un profil.

```sh
ferr profile show onset
```

#### profile delete

Supprime un profil.

```sh
ferr profile delete onset
```

---

### history

Consulte l'historique des sessions de copie (base SQLite dans `~/.local/share/ferr/history.db`).

```
ferr history <SOUS-COMMANDE>
```

#### history list

Liste les sessions récentes.

```
ferr history list [OPTIONS]
```

| Option | Type | Défaut | Description |
|--------|------|--------|-------------|
| `--limit <N>` | Entier | `20` | Nombre de sessions à afficher |
| `--since <DATE>` | RFC 3339 | — | Filtre : sessions depuis cette date |

```sh
ferr history list
ferr history list --limit 50 --since 2025-01-01T00:00:00Z
```

#### history show

Affiche le détail JSON d'une session.

```sh
ferr history show 42
```

#### history find

Recherche un fichier par hash ou nom.

```sh
ferr history find abcdef1234567890
ferr history find A001_C001.braw
```

---

## Codes de sortie

| Code | Signification |
|------|--------------|
| 0 | Succès |
| 1 | Avertissement (manquants, bit rot détecté) |
| 2 | Erreur fatale |
| 3 | Irrécupérable (PAR2 insuffisant) |

---

## Formats de fichiers

### Manifest (`ferr-manifest.json`)

Généré automatiquement dans chaque destination après une copie. Contient :

```json
{
  "ferr_version": "0.1.0",
  "generated_at": "2025-06-15T14:23:00Z",
  "hostname": "dit-macbook",
  "source_path": "/Volumes/A001",
  "total_files": 24,
  "total_size_bytes": 107374182400,
  "duration_secs": 358.4,
  "status": "Ok",
  "files": [
    {
      "path": "A001_C001.braw",
      "size": 4294967296,
      "hash_algo": "xxhash64",
      "hash": "a1b2c3d4e5f6g7h8",
      "modified_at": "2025-06-15T09:15:00Z",
      "status": "Ok",
      "par2_generated": true
    }
  ]
}
```

### Profil (`~/.config/ferr/profiles/<nom>.json`)

```json
{
  "name": "onset",
  "created_at": "2025-06-01T08:00:00Z",
  "destinations": ["/mnt/ssd1", "/mnt/ssd2"],
  "hash_algo": "sha256",
  "par2_redundancy": 10,
  "camera_mode": true,
  "auto_eject": true
}
```

---

## Modèles de renommage

L'option `--rename` accepte un modèle avec les variables suivantes :

| Variable | Description | Exemple |
|----------|-------------|---------|
| `{date}` | Date de modification (YYYYMMDD) | `20250615` |
| `{camera}` | Identifiant caméra détecté | `A` |
| `{reel}` | Numéro de bobine | `001` |
| `{clip}` | Nom du clip sans extension | `A001_C001` |
| `{ext}` | Extension avec point | `.braw` |
| `{original}` | Nom de fichier original complet | `A001_C001.braw` |

**Exemple**

```sh
ferr copy /Volumes/A001 /backup --rename "{date}_{camera}_{reel}_{clip}{ext}"
# → 20250615_A_001_A001_C001.braw
```

---

## Profils

Les profils permettent de mémoriser un ensemble d'options et de les réutiliser :

```sh
# Créer un profil "tournage" pour les jours de tournage
ferr profile save tournage \
  --dest /mnt/raid1 \
  --dest /mnt/raid2 \
  --hash sha256 \
  --par2 10 \
  --camera \
  --eject

# Utiliser le profil
ferr copy /Volumes/CARD /ignored --profile tournage
ferr watch /Volumes --profile tournage
```

Quand un profil est chargé, ses destinations et options remplacent celles de la ligne de commande (sauf `--rename` qui est toujours pris en compte).

---

## Historique des sessions

Chaque copie réussie est enregistrée dans une base SQLite locale. Cela permet :

- **Déduplication** (`ferr copy --dedup`) : saute les fichiers dont le hash exact est déjà présent dans l'historique
- **Audit** : retrouver quand et vers où un fichier a été copié
- **Recherche** : retrouver un fichier par son hash ou son nom

```sh
# Activer la déduplication
ferr copy /Volumes/A002 /backup --dedup

# Trouver où un fichier a déjà été copié
ferr history find A001_C001.braw

# Voir le détail d'une session
ferr history show 7
```

---

*ferr v0.1.0 — MIT License*
