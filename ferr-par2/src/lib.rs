//! ferr-par2 — génération, vérification et réparation PAR2 via subprocess par2cmdline.
//!
//! # Dépendance système
//! Requiert le binaire `par2` (par2cmdline ≥ 0.8) ou les binaires legacy
//! `par2create` / `par2verify` / `par2repair`.
//!
//! Installation :
//! - macOS  : `brew install par2`
//! - Debian : `apt install par2`
//! - Windows: `winget install par2cmdline`
//!
//! # Stub
//! Si la variable d'environnement `FERR_PAR2_STUB=1` est définie à la compilation
//! (via cargo), toutes les fonctions retournent une erreur explicative sans
//! tenter d'invoquer le binaire. Utile pour les tests CI sans par2 installé.

use std::path::{Path, PathBuf};
use std::process::Stdio;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Résultat d'une génération PAR2.
#[derive(Debug)]
pub struct Par2Result {
    /// Fichiers .par2 générés (index + volumes).
    pub par2_files: Vec<PathBuf>,
    /// Taille totale des données de redondance en octets.
    pub total_redundancy_bytes: u64,
}

/// Statut retourné par `verify()`.
#[derive(Debug, PartialEq, Eq)]
pub enum Par2VerifyStatus {
    /// Tous les fichiers sont corrects.
    Ok,
    /// Des fichiers sont corrompus mais réparables.
    Repairable,
    /// Des fichiers sont corrompus et irréparables.
    Unrecoverable,
}

/// Statut retourné par `repair()`.
#[derive(Debug, PartialEq, Eq)]
pub enum Par2RepairStatus {
    /// Réparation réussie.
    Repaired,
    /// Réparation impossible (blocs insuffisants).
    Failed,
}

// ---------------------------------------------------------------------------
// Détection du binaire par2
// ---------------------------------------------------------------------------

/// Style du binaire par2cmdline installé.
#[derive(Clone, Copy)]
enum Par2Style {
    /// Binaire unique moderne : `par2 create|verify|repair …`
    Modern,
    /// Binaires séparés legacy : `par2create`, `par2verify`, `par2repair`
    Legacy,
}

/// Détecte quel binaire par2 est disponible.
/// Retourne `None` si aucun binaire n'est trouvé.
fn find_par2_binary() -> Option<Par2Style> {
    // Essai moderne (`par2 --help`)
    if std::process::Command::new("par2")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|_| true)
        .unwrap_or(false)
    {
        return Some(Par2Style::Modern);
    }
    // Essai legacy (`par2create --help`)
    if std::process::Command::new("par2create")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|_| true)
        .unwrap_or(false)
    {
        return Some(Par2Style::Legacy);
    }
    None
}

/// Retourne un message d'erreur d'installation clair.
fn par2_not_found_error() -> anyhow::Error {
    anyhow::anyhow!(
        "par2cmdline introuvable. Installez-le :\n\
         • macOS  : brew install par2\n\
         • Debian : sudo apt install par2\n\
         • Windows: winget install par2cmdline"
    )
}

// ---------------------------------------------------------------------------
// Collecte des fichiers sources (sans les .par2)
// ---------------------------------------------------------------------------

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_rec(dir, &mut files);
    files.sort(); // reproductibilité
    files
}

fn collect_files_rec(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            // Ne pas descendre dans un dossier _par2 déjà créé
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name == "_par2" {
                continue;
            }
            collect_files_rec(&path, out);
        } else if path.is_file() {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            // Exclure les fichiers meta ferr
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if ext == "par2" || name == "ferr-manifest.json" || ext == "pdf" {
                continue;
            }
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing de la sortie par2cmdline
// ---------------------------------------------------------------------------

/// Extrait un pourcentage depuis une ligne comme "Repairing: 45.2%"
fn parse_pct(line: &str) -> Option<u8> {
    let pct_idx = line.find('%')?;
    let before = &line[..pct_idx];
    let token = before.split_whitespace().last()?;
    // Accepte "45" ou "45.2"
    let val: f32 = token.parse().ok()?;
    Some(val.clamp(0.0, 100.0) as u8)
}

/// Découpe la sortie par2 en tokens en splitant sur '\n' et '\r'
/// (par2 utilise '\r' pour les mises à jour de progression sur terminal).
fn split_par2_output(raw: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(raw)
        .split(|c: char| c == '\n' || c == '\r')
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

/// Génère des fichiers PAR2 pour tous les fichiers de `source_dir`.
///
/// Les fichiers `.par2` sont écrits dans `output_dir` (créé si nécessaire).
/// `redundancy_pct` doit être compris entre 1 et 40.
/// `on_progress` est appelé avec un pourcentage (0–100).
pub fn generate(
    source_dir: &Path,
    output_dir: &Path,
    redundancy_pct: u8,
    mut on_progress: impl FnMut(u8),
) -> anyhow::Result<Par2Result> {
    if !(1..=40).contains(&redundancy_pct) {
        anyhow::bail!(
            "redundancy_pct doit être entre 1 et 40, reçu {}",
            redundancy_pct
        );
    }

    // ── Stub ──────────────────────────────────────────────────────────────
    #[cfg(par2_stub)]
    {
        on_progress(0);
        anyhow::bail!(
            "ferr-par2: génération PAR2 désactivée (stub). \
             Supprimez FERR_PAR2_STUB et installez par2cmdline."
        );
    }

    // ── Implémentation subprocess ─────────────────────────────────────────
    #[cfg(not(par2_stub))]
    {
        let style = find_par2_binary().ok_or_else(par2_not_found_error)?;

        let files = collect_files(source_dir);
        if files.is_empty() {
            anyhow::bail!("Aucun fichier source trouvé dans {}", source_dir.display());
        }

        std::fs::create_dir_all(output_dir)?;

        let basename = source_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "ferr".to_string());
        let index_file = output_dir.join(format!("{basename}.par2"));

        on_progress(0);

        // Construire la commande
        // par2 create -r{pct} -n1 -B{src} {index} [files…]
        let mut cmd = match style {
            Par2Style::Modern => {
                let mut c = std::process::Command::new("par2");
                c.arg("create");
                c
            }
            Par2Style::Legacy => std::process::Command::new("par2create"),
        };

        cmd.arg(format!("-r{redundancy_pct}"))
            .arg("-n1") // un seul volume de récupération
            .arg(format!("-B{}", source_dir.display()))
            .arg(&index_file);

        for f in &files {
            cmd.arg(f);
        }

        // Utiliser output() pour éviter le deadlock lié aux pipes :
        // par2 écrit la progression via \r (carriage return), pas \n,
        // ce qui remplit le buffer de la pipe avant que les lignes soient lues.
        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Impossible de lancer par2 : {e}"))?;

        // Rejouer les tokens "Opening:" pour un progrès approximatif.
        let tokens = split_par2_output(&output.stdout);
        let total = files.len().max(1) as f32;
        let open_count = tokens
            .iter()
            .filter(|l| l.trim().starts_with("Opening:"))
            .count() as f32;
        for (i, _) in tokens
            .iter()
            .filter(|l| l.trim().starts_with("Opening:"))
            .enumerate()
        {
            let pct = ((i + 1) as f32 / open_count.max(1.0) * total.min(open_count) / total * 80.0)
                .min(80.0) as u8;
            on_progress(pct);
        }

        if !output.status.success() {
            let stderr_msg = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "par2 create a échoué (code {:?}): {}",
                output.status.code(),
                stderr_msg.trim()
            );
        }

        on_progress(100);

        // Collecter les fichiers .par2 générés
        let par2_files: Vec<PathBuf> = std::fs::read_dir(output_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "par2").unwrap_or(false))
            .collect();

        // Estimer la taille de redondance
        let source_size: u64 = files
            .iter()
            .map(|f| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0))
            .sum();
        let total_redundancy_bytes = (source_size as f64 * redundancy_pct as f64 / 100.0) as u64;

        Ok(Par2Result {
            par2_files,
            total_redundancy_bytes,
        })
    }
}

/// Vérifie l'intégrité des fichiers couverts par `par2_index`.
///
/// `par2_index` : chemin vers le fichier `.par2` index (ex. `_par2/ferr.par2`).
/// `target_dir` : répertoire contenant les fichiers originaux.
pub fn verify(par2_index: &Path, target_dir: &Path) -> anyhow::Result<Par2VerifyStatus> {
    // ── Stub ──────────────────────────────────────────────────────────────
    #[cfg(par2_stub)]
    {
        anyhow::bail!("ferr-par2: vérification PAR2 désactivée (stub).");
    }

    // ── Implémentation subprocess ─────────────────────────────────────────
    #[cfg(not(par2_stub))]
    {
        let style = find_par2_binary().ok_or_else(par2_not_found_error)?;

        let mut cmd = match style {
            Par2Style::Modern => {
                let mut c = std::process::Command::new("par2");
                c.arg("verify");
                c
            }
            Par2Style::Legacy => std::process::Command::new("par2verify"),
        };

        cmd.arg(format!("-B{}", target_dir.display()))
            .arg(par2_index)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Impossible de lancer par2verify : {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}{stderr}");

        // par2cmdline imprime ces chaînes fixes sur stdout.
        // IMPORTANT : tester "not possible" AVANT "possible" car l'un contient l'autre.
        let c = combined.to_lowercase();
        if c.contains("repair is not required") || c.contains("all files are correct") {
            Ok(Par2VerifyStatus::Ok)
        } else if c.contains("repair is not possible")
            || c.contains("cannot be repaired")
            || c.contains("you need")
        {
            Ok(Par2VerifyStatus::Unrecoverable)
        } else if c.contains("repair is possible")
            || c.contains("repair is required")
            || c.contains("repairable")
        {
            Ok(Par2VerifyStatus::Repairable)
        } else if output.status.success() {
            // Code de retour 0 sans message clair → OK
            Ok(Par2VerifyStatus::Ok)
        } else {
            // Code ≥ 1 sans message clair → irréparable
            Ok(Par2VerifyStatus::Unrecoverable)
        }
    }
}

/// Tente de réparer les fichiers corrompus identifiés par `par2_index`.
///
/// `on_progress` est appelé avec un pourcentage (0–100).
pub fn repair(
    par2_index: &Path,
    target_dir: &Path,
    mut on_progress: impl FnMut(u8),
) -> anyhow::Result<Par2RepairStatus> {
    on_progress(0);

    // ── Stub ──────────────────────────────────────────────────────────────
    #[cfg(par2_stub)]
    {
        anyhow::bail!("ferr-par2: réparation PAR2 désactivée (stub).");
    }

    // ── Implémentation subprocess ─────────────────────────────────────────
    #[cfg(not(par2_stub))]
    {
        let style = find_par2_binary().ok_or_else(par2_not_found_error)?;

        let mut cmd = match style {
            Par2Style::Modern => {
                let mut c = std::process::Command::new("par2");
                c.arg("repair");
                c
            }
            Par2Style::Legacy => std::process::Command::new("par2repair"),
        };

        cmd.arg(format!("-B{}", target_dir.display()))
            .arg(par2_index);

        let output = cmd
            .output()
            .map_err(|e| anyhow::anyhow!("Impossible de lancer par2repair : {e}"))?;

        // Rejouer les pourcentages depuis la sortie collectée.
        // par2 imprime "Repairing: 45.2%" avec \r — on split sur \r|\n.
        let mut last_pct: u8 = 0;
        for token in split_par2_output(&output.stdout) {
            if let Some(pct) = parse_pct(&token) {
                if pct > last_pct {
                    on_progress(pct);
                    last_pct = pct;
                }
            }
        }
        on_progress(100);

        // par2repair : code 0 = réparé, ≠0 = irréparable ou erreur
        if output.status.success() {
            Ok(Par2RepairStatus::Repaired)
        } else {
            Ok(Par2RepairStatus::Failed)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Tests unitaires (ne nécessitent pas par2) ─────────────────────────

    #[test]
    fn generate_invalid_redundancy_below_1() {
        let result = generate(Path::new("/tmp"), Path::new("/tmp"), 0, |_| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("entre 1 et 40"));
    }

    #[test]
    fn generate_invalid_redundancy_above_40() {
        let result = generate(Path::new("/tmp"), Path::new("/tmp"), 41, |_| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("entre 1 et 40"));
    }

    #[test]
    fn parse_pct_various_formats() {
        assert_eq!(parse_pct("Repairing: 45.2%"), Some(45));
        assert_eq!(parse_pct("Repairing: 100%"), Some(100));
        assert_eq!(parse_pct("Progress: 0%"), Some(0));
        assert_eq!(parse_pct("no percentage here"), None);
    }

    #[test]
    fn collect_files_excludes_par2_and_manifest() {
        let dir = std::env::temp_dir().join("ferr_par2_collect_test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("clip.braw"), b"video").unwrap();
        std::fs::write(dir.join("ferr-manifest.json"), b"{}").unwrap();
        std::fs::write(dir.join("file.par2"), b"par2data").unwrap();
        std::fs::write(dir.join("report.pdf"), b"pdf").unwrap();

        let files = collect_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("clip.braw"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn collect_files_skips_par2_subdir() {
        let dir = std::env::temp_dir().join("ferr_par2_skip_test");
        let par2_dir = dir.join("_par2");
        std::fs::create_dir_all(&par2_dir).unwrap();
        std::fs::write(dir.join("clip.braw"), b"video").unwrap();
        std::fs::write(par2_dir.join("clip.par2"), b"par2data").unwrap();

        let files = collect_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("clip.braw"));

        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Tests stub (actifs quand FERR_PAR2_STUB=1 à la compilation) ───────

    #[cfg(par2_stub)]
    #[test]
    fn generate_stub_returns_error() {
        let result = generate(Path::new("/tmp"), Path::new("/tmp"), 10, |_| {});
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("stub"));
    }

    #[cfg(par2_stub)]
    #[test]
    fn verify_stub_returns_error() {
        let result = verify(Path::new("/tmp/test.par2"), Path::new("/tmp"));
        assert!(result.is_err());
    }

    #[cfg(par2_stub)]
    #[test]
    fn repair_stub_returns_error() {
        let result = repair(Path::new("/tmp/test.par2"), Path::new("/tmp"), |_| {});
        assert!(result.is_err());
    }

    // ── Tests d'intégration réels (nécessitent par2 installé) ─────────────
    // Ignorés par défaut ; lancer avec : cargo test -- --ignored

    #[test]
    #[ignore = "nécessite par2cmdline installé sur le système"]
    fn generate_verify_repair_roundtrip() {
        let base = std::env::temp_dir().join("ferr_par2_integration");
        let src = base.join("src");
        let par2 = base.join("_par2");
        // Nettoyer les restes d'un run précédent avant de démarrer
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&src).unwrap();

        // Créer des fichiers sources de test (512 Ko chacun — assez grands pour
        // que la réparation de quelques octets soit bien dans les 10% de redondance)
        let file_size = 512 * 1024usize;
        for i in 0..4u8 {
            let content: Vec<u8> = (0..file_size)
                .map(|j| ((j + i as usize) % 256) as u8)
                .collect();
            std::fs::write(src.join(format!("file{i:02}.dat")), &content).unwrap();
        }

        // Générer les fichiers PAR2
        let mut progress_calls = 0u32;
        let result = generate(&src, &par2, 10, |p| {
            assert!(p <= 100);
            progress_calls += 1;
        });
        assert!(result.is_ok(), "generate échoué : {:?}", result.err());
        let r = result.unwrap();
        assert!(!r.par2_files.is_empty(), "Aucun fichier .par2 généré");
        assert!(progress_calls > 0);

        // Vérifier (tous les fichiers OK)
        let index = par2.join("src.par2");
        let status = verify(&index, &src).unwrap();
        assert_eq!(status, Par2VerifyStatus::Ok);

        // Corrompre quelques octets IN-PLACE (taille inchangée — crucial pour la réparation)
        let corrupt_path = src.join("file00.dat");
        let mut data = std::fs::read(&corrupt_path).unwrap();
        // Flip les 64 premiers octets
        for b in data[..64].iter_mut() {
            *b ^= 0xFF;
        }
        std::fs::write(&corrupt_path, &data).unwrap();

        // Vérifier (doit détecter la corruption)
        let status = verify(&index, &src).unwrap();
        assert_ne!(
            status,
            Par2VerifyStatus::Ok,
            "La corruption n'a pas été détectée"
        );

        // Réparer
        let mut repair_progress = 0u8;
        let repair_status = repair(&index, &src, |p| {
            repair_progress = p;
        })
        .unwrap();
        assert_eq!(
            repair_status,
            Par2RepairStatus::Repaired,
            "Réparation échouée"
        );
        assert_eq!(repair_progress, 100);

        // Revérifier (OK après réparation)
        let status = verify(&index, &src).unwrap();
        assert_eq!(
            status,
            Par2VerifyStatus::Ok,
            "Toujours corrompu après réparation"
        );

        std::fs::remove_dir_all(&base).ok();
    }
}
