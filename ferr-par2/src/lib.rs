//! # Fonctionnement
//! - **Génération** : Requiert le binaire externe `par2` (subprocess).
//! - **Vérification / Réparation** : Native via la bibliothèque `rust_par2` (aucun binaire requis).

use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Crée un lien dur (hard link) vers `src` à l'emplacement `dest`.
/// Si le hard link échoue (systèmes de fichiers différents, droits
/// insuffisants), replie sur une copie du fichier.
fn link_or_copy(src: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::hard_link(src, dest).or_else(|_| std::fs::copy(src, dest).map(|_| ()))
}

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
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name == "_par2" || name.starts_with("_ferr_logs_") {
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
            if ext == "par2" || name == "ferr-manifest.json" || name.starts_with("_ferr_logs_") || ext == "pdf" {
                continue;
            }
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing de la sortie par2cmdline
// ---------------------------------------------------------------------------

/// Découpe la sortie par2 en tokens en splitant sur '\n' et '\r'
/// (par2 utilise '\r' pour les mises à jour de progression sur terminal).
fn split_par2_output(raw: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(raw)
        .split(['\n', '\r'])
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
    // Créer une vue consolidée (data + parity) via des symlinks temporaires
    let view = Par2View::create(par2_index, target_dir)?;

    let file_set = rust_par2::parse(par2_index)
        .map_err(|e| anyhow::anyhow!("Échec du parsing du fichier PAR2 : {:?}", e))?;

    let result = rust_par2::verify(&file_set, &view.path);

    if result.all_correct() {
        Ok(Par2VerifyStatus::Ok)
    } else if result.repair_possible {
        Ok(Par2VerifyStatus::Repairable)
    } else {
        Ok(Par2VerifyStatus::Unrecoverable)
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

    // Créer une vue consolidée (data + parity) via des symlinks temporaires
    let view = Par2View::create(par2_index, target_dir)?;

    let file_set = rust_par2::parse(par2_index)
        .map_err(|e| anyhow::anyhow!("Échec du parsing du fichier PAR2 : {:?}", e))?;

    on_progress(10);

    // rust_par2 écrit dans les fichiers ; comme ce sont des symlinks vers target_dir,
    // la réparation se fait "in-place" sur les originaux.
    match rust_par2::repair(&file_set, &view.path) {
        Ok(_) => {
            on_progress(100);
            Ok(Par2RepairStatus::Repaired)
        }
        Err(e) => {
            tracing::error!("erreur de réparation native : {:?}", e);
            Ok(Par2RepairStatus::Failed)
        }
    }
}

// ---------------------------------------------------------------------------
// Par2View : Vue consolidée par symlinks
// ---------------------------------------------------------------------------

struct Par2View {
    path: PathBuf,
}

impl Par2View {
    fn create(par2_index: &Path, target_dir: &Path) -> anyhow::Result<Self> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let view_path = std::env::temp_dir().join(format!("par2_view_{}", timestamp));
        std::fs::create_dir_all(&view_path)?;

        // 1. Symlinks vers les fichiers de donnée (RECURSIF)
        //    Il faut recréer la structure de dossiers pour que rust_par2
        //    retrouve "folder/file.dat".
        Self::link_dir_rec(target_dir, &view_path, target_dir)?;

        // 2. Symlinks vers les fichiers PAR2 (index + volumes)
        if let Some(par2_dir) = par2_index.parent() {
            for entry in std::fs::read_dir(par2_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "par2") {
                    let name = entry.file_name();
                    let dest = view_path.join(&name);
                    if !dest.exists() {
                        let _ = link_or_copy(&path, &dest);
                    }
                }
            }
        }

        Ok(Self { path: view_path })
    }

    fn link_dir_rec(src_dir: &Path, view_root: &Path, target_root: &Path) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(src_dir)? {
            let entry = entry?;
            let path = entry.path();
            let rel_path = path.strip_prefix(target_root)?;
            let dest = view_root.join(rel_path);

            if path.is_dir() {
                // Éviter de boucler ou de descendre dans _par2 si on est à la racine
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
                if name == "_par2" || name.starts_with("_ferr_logs_") {
                    continue;
                }
                std::fs::create_dir_all(&dest)?;
                Self::link_dir_rec(&path, view_root, target_root)?;
            } else if path.is_file() {
                // Ne pas symlinker les fichiers qu'on veut ignorer (le pdf de rapport, etc)
                // si on veut être strict, mais ici on peut tout lier, rust_par2
                // ne s'intéressera qu'à ce qu'il connaît.
                let _ = link_or_copy(&path, &dest);
            }
        }
        Ok(())
    }
}

impl Drop for Par2View {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
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
