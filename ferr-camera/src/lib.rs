use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraFormat {
    Braw,
    R3D,
    ArriMxf,
    SonyXocn,
    CanonXf,
    ProRes,
    Unknown,
}

impl std::fmt::Display for CameraFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraFormat::Braw => write!(f, "BRAW (Blackmagic)"),
            CameraFormat::R3D => write!(f, "R3D (RED)"),
            CameraFormat::ArriMxf => write!(f, "MXF (ARRI)"),
            CameraFormat::SonyXocn => write!(f, "XOCN (Sony)"),
            CameraFormat::CanonXf => write!(f, "XF-AVC (Canon)"),
            CameraFormat::ProRes => write!(f, "ProRes (MOV)"),
            CameraFormat::Unknown => write!(f, "Inconnu"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClipMetadata {
    pub timecode_in: Option<String>,
    pub timecode_out: Option<String>,
    pub framerate: Option<String>,
    pub resolution: Option<String>,
    pub camera_id: Option<String>,
    pub reel: Option<String>,
    pub iso: Option<u32>,
    pub white_balance: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub format: CameraFormat,
    pub primary_file: PathBuf,
    pub sidecar_files: Vec<PathBuf>,
    pub total_size: u64,
    pub metadata: ClipMetadata,
}

#[derive(Debug)]
pub struct ClipIntegrity {
    pub clip_id: String,
    pub internal_ok: bool,
    pub all_parts_ok: bool,
    pub missing_parts: Vec<PathBuf>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// detect_camera_format
// ---------------------------------------------------------------------------

/// Détecte le format caméra d'un dossier ou fichier.
/// Ordre de priorité : structure dossier > extension fichier.
pub fn detect_camera_format(path: &Path) -> Option<CameraFormat> {
    if path.is_dir() {
        // Détection par sous-dossiers caractéristiques
        if path.join("XDROOT").exists() {
            return Some(CameraFormat::SonyXocn);
        }
        if path.join("CANONMXF").exists() || path.join("CONTENTS").join("CLIPS001").exists() {
            return Some(CameraFormat::CanonXf);
        }
        if has_arri_structure(path) {
            return Some(CameraFormat::ArriMxf);
        }
        // Détection par extension majoritaire dans le dossier
        return detect_by_dominant_extension(path);
    }

    // Fichier individuel
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "braw" => Some(CameraFormat::Braw),
        "r3d" => Some(CameraFormat::R3D),
        "mxf" => Some(CameraFormat::ArriMxf), // heuristique
        "mov" => Some(CameraFormat::ProRes),
        _ => Some(CameraFormat::Unknown),
    }
}

fn has_arri_structure(path: &Path) -> bool {
    // Recherche d'un sous-dossier ARRI ou d'un fichier .mxf avec structure ARRI
    if path.join("ARRI").exists() {
        return true;
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("ARRI").exists() {
                return true;
            }
        }
    }
    false
}

fn detect_by_dominant_extension(dir: &Path) -> Option<CameraFormat> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    *counts.entry(ext_lower).or_insert(0) += 1;
                }
            }
        }
    }
    let dominant = counts.into_iter().max_by_key(|(_, v)| *v)?;
    match dominant.0.as_str() {
        "braw" => Some(CameraFormat::Braw),
        "r3d" => Some(CameraFormat::R3D),
        "mxf" => Some(CameraFormat::ArriMxf),
        "mov" => Some(CameraFormat::ProRes),
        _ => Some(CameraFormat::Unknown),
    }
}

// ---------------------------------------------------------------------------
// scan_clips
// ---------------------------------------------------------------------------

/// Scanne les clips dans un dossier source.
/// Si `format` est None, auto-détecte.
pub fn scan_clips(source: &Path, format: Option<CameraFormat>) -> anyhow::Result<Vec<Clip>> {
    let fmt = format
        .or_else(|| detect_camera_format(source))
        .unwrap_or(CameraFormat::Unknown);

    match fmt {
        CameraFormat::Braw => scan_braw(source),
        CameraFormat::R3D => scan_r3d(source),
        CameraFormat::ArriMxf => scan_mxf(source, CameraFormat::ArriMxf),
        CameraFormat::SonyXocn => scan_xocn(source),
        CameraFormat::CanonXf => scan_mxf(source, CameraFormat::CanonXf),
        CameraFormat::ProRes => scan_prores(source),
        CameraFormat::Unknown => scan_generic(source),
    }
}

fn scan_braw(source: &Path) -> anyhow::Result<Vec<Clip>> {
    let mut clips = Vec::new();
    for entry in collect_files_by_ext(source, "braw")? {
        let meta = parse_braw_name(&entry);
        let size = file_size(&entry);
        let id = entry
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        // Chercher un sidecar (.sidecar ou .braw.sidecar)
        let sidecar = entry.with_extension("sidecar");
        let mut sidecars = Vec::new();
        if sidecar.exists() {
            sidecars.push(sidecar);
        }
        clips.push(Clip {
            id,
            format: CameraFormat::Braw,
            primary_file: entry,
            sidecar_files: sidecars,
            total_size: size,
            metadata: meta,
        });
    }
    Ok(clips)
}

fn scan_r3d(source: &Path) -> anyhow::Result<Vec<Clip>> {
    let mut clips: HashMap<String, Clip> = HashMap::new();
    for entry in collect_files_by_ext(source, "r3d")? {
        let stem = entry
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        // Détecter les chunks : A001_C001_001.r3d → base = A001_C001
        let base = if let Some(pos) = stem.rfind('_') {
            let suffix = &stem[pos + 1..];
            if suffix.chars().all(|c| c.is_ascii_digit()) {
                stem[..pos].to_string()
            } else {
                stem.clone()
            }
        } else {
            stem.clone()
        };

        let size = file_size(&entry);
        if let Some(clip) = clips.get_mut(&base) {
            clip.sidecar_files.push(entry);
            clip.total_size += size;
        } else {
            let meta = parse_r3d_name(&base);
            clips.insert(
                base.clone(),
                Clip {
                    id: base,
                    format: CameraFormat::R3D,
                    primary_file: entry,
                    sidecar_files: Vec::new(),
                    total_size: size,
                    metadata: meta,
                },
            );
        }
    }
    Ok(clips.into_values().collect())
}

fn scan_mxf(source: &Path, format: CameraFormat) -> anyhow::Result<Vec<Clip>> {
    let mut clips = Vec::new();
    for entry in collect_files_by_ext(source, "mxf")? {
        let id = entry
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        let size = file_size(&entry);
        clips.push(Clip {
            id,
            format: format.clone(),
            primary_file: entry,
            sidecar_files: Vec::new(),
            total_size: size,
            metadata: ClipMetadata::default(),
        });
    }
    Ok(clips)
}

fn scan_xocn(source: &Path) -> anyhow::Result<Vec<Clip>> {
    // XDROOT structure
    let xdroot = source.join("XDROOT");
    let base = if xdroot.exists() {
        xdroot
    } else {
        source.to_path_buf()
    };
    scan_mxf(&base, CameraFormat::SonyXocn)
}

fn scan_prores(source: &Path) -> anyhow::Result<Vec<Clip>> {
    let mut clips = Vec::new();
    for entry in collect_files_by_ext(source, "mov")? {
        let id = entry
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".into());
        let size = file_size(&entry);
        clips.push(Clip {
            id,
            format: CameraFormat::ProRes,
            primary_file: entry,
            sidecar_files: Vec::new(),
            total_size: size,
            metadata: ClipMetadata::default(),
        });
    }
    Ok(clips)
}

fn scan_generic(source: &Path) -> anyhow::Result<Vec<Clip>> {
    let mut clips = Vec::new();
    let mut files = Vec::new();
    collect_all_files(source, &mut files)?;
    for entry in files {
        if entry.is_file() {
            let id = entry
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into());
            let size = file_size(&entry);
            clips.push(Clip {
                id,
                format: CameraFormat::Unknown,
                primary_file: entry,
                sidecar_files: Vec::new(),
                total_size: size,
                metadata: ClipMetadata::default(),
            });
        }
    }
    Ok(clips)
}

// ---------------------------------------------------------------------------
// verify_clip_integrity
// ---------------------------------------------------------------------------

pub fn verify_clip_integrity(clip: &Clip) -> anyhow::Result<ClipIntegrity> {
    let mut missing = Vec::new();

    // Fichier principal
    if !clip.primary_file.exists() {
        missing.push(clip.primary_file.clone());
    }
    // Sidecars
    for sidecar in &clip.sidecar_files {
        if !sidecar.exists() {
            missing.push(sidecar.clone());
        }
    }

    let all_parts_ok = missing.is_empty();

    // Vérification interne basique : taille non nulle pour le fichier principal
    let internal_ok = if clip.primary_file.exists() {
        file_size(&clip.primary_file) > 0
    } else {
        false
    };

    Ok(ClipIntegrity {
        clip_id: clip.id.clone(),
        internal_ok,
        all_parts_ok,
        missing_parts: missing,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// apply_rename_template  (feat 15)
// ---------------------------------------------------------------------------

/// Applique un template de renommage à un clip.
///
/// Tokens disponibles :
/// {date} {camera} {reel} {clip} {ext} {original}
pub fn apply_rename_template(clip: &Clip, template: &str) -> anyhow::Result<String> {
    let ext = clip
        .primary_file
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let original = clip
        .primary_file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| clip.id.clone());

    // Extraire date de clip.id si non disponible dans metadata
    let date_str = extract_date_from_id(&clip.id);

    let result = template
        .replace("{date}", &date_str)
        .replace(
            "{camera}",
            clip.metadata
                .camera_id
                .as_deref()
                .unwrap_or(&clip.id[..clip.id.len().min(4)]),
        )
        .replace("{reel}", clip.metadata.reel.as_deref().unwrap_or("XXXX"))
        .replace("{clip}", &clip.id)
        .replace("{ext}", &ext)
        .replace("{original}", &original);

    Ok(result)
}

fn extract_date_from_id(id: &str) -> String {
    // Tente d'extraire YYYYMMDD ou YYMMDD depuis le nom
    // Convention BRAW : A001_C001_250328 → date = 20250328
    let parts: Vec<&str> = id.split('_').collect();
    for part in &parts {
        if part.len() == 6 && part.chars().all(|c| c.is_ascii_digit()) {
            // YYMMDD → 20YYMMDD
            return format!("20{part}");
        }
        if part.len() == 8 && part.chars().all(|c| c.is_ascii_digit()) {
            return part.to_string();
        }
    }
    "00000000".to_string()
}

// ---------------------------------------------------------------------------
// Parsing des noms de fichiers (métadonnées)
// ---------------------------------------------------------------------------

/// Convention BRAW : A001_C001_250328_BRAW.braw
fn parse_braw_name(path: &Path) -> ClipMetadata {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    parse_convention_name(&stem)
}

/// Convention R3D : A001_C001.r3d
fn parse_r3d_name(base: &str) -> ClipMetadata {
    parse_convention_name(base)
}

/// Convention générique : <CAMID>_<CLIPID>_<DATE>_...
fn parse_convention_name(name: &str) -> ClipMetadata {
    let parts: Vec<&str> = name.split('_').collect();
    let mut meta = ClipMetadata::default();

    if let Some(cam) = parts.first() {
        // Camera ID : lettre + 3 chiffres (ex: A001)
        if cam.len() == 4
            && cam
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && cam[1..].chars().all(|c| c.is_ascii_digit())
        {
            meta.camera_id = Some(cam.to_string());
        }
    }
    if parts.len() >= 2 {
        // Reel : lettre + 3 chiffres (ex: R1AB, A001)
        if let Some(r) = parts.get(1) {
            if r.len() >= 4 {
                meta.reel = Some(r.to_string());
            }
        }
    }
    meta
}

// ---------------------------------------------------------------------------
// Utilitaires
// ---------------------------------------------------------------------------

fn collect_files_by_ext(dir: &Path, ext: &str) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_by_ext_recursive(dir, ext, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_by_ext_recursive(
    dir: &Path,
    ext: &str,
    out: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_by_ext_recursive(&path, ext, out)?;
        } else if path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase() == ext)
            .unwrap_or(false)
        {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_all_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_all_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferr_camera_{name}"));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn detect_braw_by_extension() {
        let base = tmp("braw_detect");
        let f = base.join("A001_C001_250328.braw");
        std::fs::write(&f, b"fake braw").unwrap();
        let fmt = detect_camera_format(&f).unwrap();
        assert_eq!(fmt, CameraFormat::Braw);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scan_braw_clips() {
        let base = tmp("braw_scan");
        std::fs::write(base.join("A001_C001_250328.braw"), b"fake braw 1").unwrap();
        std::fs::write(base.join("A001_C002_250328.braw"), b"fake braw 2").unwrap();
        std::fs::write(base.join("A001_C001_250328.sidecar"), b"sidecar").unwrap();

        let clips = scan_clips(&base, Some(CameraFormat::Braw)).unwrap();
        assert_eq!(clips.len(), 2);

        let c001 = clips.iter().find(|c| {
            c.primary_file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains("C001")
        });
        assert!(c001.is_some());
        // Sidecar doit être trouvé
        assert_eq!(c001.unwrap().sidecar_files.len(), 1);

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn detect_format_by_dir_structure() {
        let base = tmp("sony_detect");
        std::fs::create_dir_all(base.join("XDROOT")).unwrap();
        let fmt = detect_camera_format(&base).unwrap();
        assert_eq!(fmt, CameraFormat::SonyXocn);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn verify_clip_all_present() {
        let base = tmp("clip_integrity");
        let f = base.join("clip.braw");
        std::fs::write(&f, b"data").unwrap();
        let clip = Clip {
            id: "clip".into(),
            format: CameraFormat::Braw,
            primary_file: f,
            sidecar_files: Vec::new(),
            total_size: 4,
            metadata: ClipMetadata::default(),
        };
        let integrity = verify_clip_integrity(&clip).unwrap();
        assert!(integrity.all_parts_ok);
        assert!(integrity.internal_ok);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn verify_clip_missing_primary() {
        let base = tmp("clip_missing");
        let clip = Clip {
            id: "clip".into(),
            format: CameraFormat::Braw,
            primary_file: base.join("nonexistent.braw"),
            sidecar_files: Vec::new(),
            total_size: 0,
            metadata: ClipMetadata::default(),
        };
        let integrity = verify_clip_integrity(&clip).unwrap();
        assert!(!integrity.all_parts_ok);
        assert_eq!(integrity.missing_parts.len(), 1);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn apply_rename_template_basic() {
        let base = tmp("rename");
        let f = base.join("A001_C001_250328.braw");
        std::fs::write(&f, b"data").unwrap();
        let clip = Clip {
            id: "A001_C001_250328".into(),
            format: CameraFormat::Braw,
            primary_file: f,
            sidecar_files: Vec::new(),
            total_size: 4,
            metadata: ClipMetadata {
                camera_id: Some("A001".into()),
                reel: Some("R1AB".into()),
                ..Default::default()
            },
        };
        let result = apply_rename_template(&clip, "{date}_{camera}_{reel}{ext}").unwrap();
        assert!(result.contains("A001"));
        assert!(result.ends_with(".braw"));
        std::fs::remove_dir_all(&base).ok();
    }
}
