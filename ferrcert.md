# Ferr Integrity Certificates (`.ferrcert`)

## Concept

A **ferr** certificate (`.ferrcert`) is a portable JSON document designed to help verify the integrity of a folder and its contents after a transfer or archiving process (for example, handing over camera rushes from set to post-production).

Rather than relying purely on separate discrete hash lists (like `.md5` or `.sha256` files) or a local session database that doesn't travel with the files, the `.ferrcert` format bundles the file hashes, folder structure, and a log of the operations into a single portable file.

---

## Technical Architecture

A `.ferrcert` file is a structured JSON document. It includes an internal hashing mechanism to help detect accidental modifications or data corruption.

### 1. Self-Signing Mechanism

In standard text-based hash files, an altered hash value might go unnoticed if the file itself is modified. The `.ferrcert` format attempts to mitigate this by including a `cert_hash` field at the root of the document.

**Calculation Process:**
1. During certificate creation or update, the system gathers all the current data (summary, file tree, events).
2. The `cert_hash` field is temporarily set to the exact string `"PLACEHOLDER"`.
3. The JSON document is serialized into a compact string deterministically (with dictionary keys sorted to ensure consistent output).
4. This complete string is hashed using SHA-256.
5. The resulting hash replaces `"PLACEHOLDER"`, and the final `.ferrcert` file is written to disk.

**Result:** Modifying the file manually—such as changing a date, an event, or a file hash—will break the total hash of the document. Running `ferr cert check <file>` recalculates this hash and returns an error (`Exit code 2`) if a modification is detected.

### 2. Event Log

The certificate is designed to be updated over time. The `events` array records the actions the dataset has been through:
- **`certified`**: Initial creation of the certificate (scanning the source, calculating initial hashes).
- **`copied`**: Logged internally when the data and its certificate are transferred using the `ferr copy` command.
- **`verified`**: Appended during a subsequent scan or check (`ferr cert verify` or the "Verify" action via the GUI). This event details whether the check passed (`PASS`) or if any issues were found.

Because appending an event recalculates the `cert_hash`, the recipient can read the transfer timeline directly from the certificate file.

### 3. Tree Structure

Instead of a flat list of hashes, the `tree` array in the certificate records:
- **Files** (with their relative path, size, and hash value, such as `xxhash64` or `sha256`).
- **Directories**, explicitly tagging **empty directories** (`empty: true`).

This distinction allows verification operations to ensure that a transfer didn't just copy the files, but also accurately preserved the expected folder structure (including empty folders which might be required by certain software workflows).

---

## Inside the JSON Structure

Here is a basic example of the `.ferrcert` document structure:

```json
{
  "ferr_cert": "1.0",
  "id": "fcert_20260408_210000_a1b2c3d4",
  "source": {
    "certified_at": "2026-04-08T19:00:00.000000Z",
    "certified_by": "ferr 0.1.0",
    "hostname": "MacBook-Pro-DIT",
    "path": "/Volumes/MAG_A001",
    "label": "MAG_A001"
  },
  "hash_algo": "xxhash64",
  "summary": {
    "total_dirs": 12,
    "total_files": 145,
    "total_bytes": 128500000000,
    "empty_dirs": 1
  },
  "tree": [
    {
      "type": "dir",
      "path": "AUDIO_SYNC/",
      "empty": true
    },
    {
      "type": "file",
      "path": "CLIP_001.braw",
      "size": 524000000,
      "hash": "a1b2c3d4e5f6g7h8..."
    }
  ],
  "events": [
    {
      "type": "certified",
      "at": "2026-04-08T19:00:00.000000Z",
      "by": "ferr 0.1.0",
      "hostname": "MacBook-Pro-DIT",
      "result": "PASS",
      "detail": "Initial certification — 145 files, 12 dirs (1 empty)"
    }
  ],
  "cert_hash": "e14b5368a1d743a60cbf6d8719c8f..."
}
```

## CLI & GUI Integration

The `.ferrcert` feature provides dedicated verification commands (`ferr cert create`, `ferr cert show`, `ferr cert check`, `ferr cert verify`).

Additionally, when running a standard copy transfer (`ferr copy`), `ferr` automatically creates a `.ferrcert` on the source, copies it along with the rest of the files, and appends a `verified` event on the destination certificate. This aims to ensure that every copy operation leaves a portable trail of integrity.
