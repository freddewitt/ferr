//! Génération de rapports PDF depuis un manifest ferr.
//!
//! Produit un rapport A4 horodaté avec résumé, tableau des fichiers copiés
//! et pied de page contenant le hash du manifest pour chaîne de contrôle.

use std::io::BufWriter;
use std::path::Path;

use ferr_hash::Hasher as _;
use printpdf::*;

const PAGE_W: f32 = 210.0; // A4 mm
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 15.0;
const LINE_H: f32 = 6.0;

/// Génère un rapport PDF horodaté depuis un manifest ferr.
pub fn generate_report(manifest: &ferr_report::Manifest, output: &Path) -> anyhow::Result<()> {
    let (doc, page1, layer1) = PdfDocument::new("ferr Report", Mm(PAGE_W), Mm(PAGE_H), "Layer 1");

    let layer = doc.get_page(page1).get_layer(layer1);
    let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;

    let mut y = PAGE_H - MARGIN;

    // --- En-tête ---
    layer.use_text(
        format!("ferr v{}  —  Rapport de copie", manifest.ferr_version),
        16.0,
        Mm(MARGIN),
        Mm(y),
        &font_bold,
    );
    y -= LINE_H * 1.5;

    layer.use_text(
        format!(
            "Généré le {}  |  Hôte : {}",
            manifest.generated_at, manifest.hostname
        ),
        9.0,
        Mm(MARGIN),
        Mm(y),
        &font_regular,
    );
    y -= LINE_H * 0.5;

    // Ligne horizontale (rectangle fin)
    let line_color = Color::Rgb(Rgb::new(0.6, 0.6, 0.6, None));
    layer.set_fill_color(line_color);
    layer.add_rect(Rect::new(
        Mm(MARGIN),
        Mm(y - 0.5),
        Mm(PAGE_W - MARGIN),
        Mm(y),
    ));
    y -= LINE_H;

    // Réinitialiser la couleur
    layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));

    // --- Résumé ---
    y -= LINE_H * 0.5;
    layer.use_text("Résumé", 12.0, Mm(MARGIN), Mm(y), &font_bold);
    y -= LINE_H;

    let summary_lines = [
        format!("Source         : {}", manifest.source_path),
        format!("Fichiers       : {}", manifest.total_files),
        format!("Taille totale  : {}", ferr_report::human_size(manifest.total_size_bytes)),
        format!("Durée          : {:.1}s", manifest.duration_secs),
        format!("Statut global  : {:?}", manifest.status),
    ];
    for line in &summary_lines {
        layer.use_text(line, 9.0, Mm(MARGIN), Mm(y), &font_regular);
        y -= LINE_H;
    }
    y -= LINE_H * 0.5;

    // --- En-tête tableau ---
    layer.use_text("Fichiers copiés", 12.0, Mm(MARGIN), Mm(y), &font_bold);
    y -= LINE_H;

    let col_w_path = 90.0_f32;
    let col_w_size = 22.0_f32;
    let col_w_hash = 38.0_f32;
    let _col_w_status = 25.0_f32;
    let x_path = MARGIN;
    let x_size = x_path + col_w_path;
    let x_hash = x_size + col_w_size;
    let x_status = x_hash + col_w_hash;

    layer.use_text("Chemin", 8.0, Mm(x_path), Mm(y), &font_bold);
    layer.use_text("Taille", 8.0, Mm(x_size), Mm(y), &font_bold);
    layer.use_text("Hash", 8.0, Mm(x_hash), Mm(y), &font_bold);
    layer.use_text("Statut", 8.0, Mm(x_status), Mm(y), &font_bold);
    y -= LINE_H * 0.8;

    // Ligne séparatrice tableau
    layer.set_fill_color(Color::Rgb(Rgb::new(0.8, 0.8, 0.8, None)));
    layer.add_rect(Rect::new(
        Mm(MARGIN),
        Mm(y),
        Mm(PAGE_W - MARGIN),
        Mm(y + 0.4),
    ));
    layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
    y -= LINE_H;

    // --- Lignes du tableau (une page max en phase 1) ---
    let mut current_page = page1;
    let mut current_layer_idx = layer1;

    for entry in &manifest.files {
        if y < MARGIN + LINE_H * 3.0 {
            // Nouvelle page
            let (new_page, new_layer) = doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
            current_page = new_page;
            current_layer_idx = new_layer;
            y = PAGE_H - MARGIN;
        }

        let layer_ref = doc.get_page(current_page).get_layer(current_layer_idx);

        let path_trunc = truncate(&entry.path, 42);
        let size_str = ferr_report::human_size(entry.size);
        let hash_trunc = truncate(&entry.hash, 16);
        let status_str = format!("{:?}", entry.status);

        layer_ref.use_text(&path_trunc, 7.5, Mm(x_path), Mm(y), &font_regular);
        layer_ref.use_text(&size_str, 7.5, Mm(x_size), Mm(y), &font_regular);
        layer_ref.use_text(&hash_trunc, 7.5, Mm(x_hash), Mm(y), &font_regular);
        layer_ref.use_text(&status_str, 7.5, Mm(x_status), Mm(y), &font_regular);
        y -= LINE_H;
    }

    // --- Pied de page (hash du manifest) ---
    let manifest_json = serde_json::to_string(manifest)?;
    let manifest_hash =
        ferr_hash::XxHasher.hash_reader(&mut std::io::Cursor::new(manifest_json.as_bytes()))?;

    let footer_layer = doc.get_page(current_page).get_layer(current_layer_idx);
    footer_layer.use_text(
        format!(
            "hash manifest : {}  |  {} — ferr v{}",
            manifest_hash.hex, manifest.generated_at, manifest.ferr_version,
        ),
        7.0,
        Mm(MARGIN),
        Mm(MARGIN),
        &font_regular,
    );

    // --- Sauvegarde ---
    let file = std::fs::File::create(output)?;
    doc.save(&mut BufWriter::new(file))?;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - (max - 1)..])
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ferr_report::Manifest {
        ferr_report::Manifest {
            ferr_version: "0.1.0".into(),
            generated_at: "2025-01-01T00:00:00Z".into(),
            hostname: "test-host".into(),
            source_path: "/footage/A001".into(),
            destinations: vec!["/backup/A001".into()],
            total_files: 2,
            total_size_bytes: 2048,
            duration_secs: 1.5,
            status: ferr_report::JobStatus::Ok,
            files: vec![
                ferr_report::FileEntry {
                    path: "A001_C001.braw".into(),
                    size: 1024,
                    hash_algo: "xxhash64".into(),
                    hash: "abcdef1234567890".into(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: ferr_report::FileStatus::Ok,
                    par2_generated: false,
                },
                ferr_report::FileEntry {
                    path: "A001_C002.braw".into(),
                    size: 1024,
                    hash_algo: "xxhash64".into(),
                    hash: "fedcba0987654321".into(),
                    modified_at: "2025-01-01T00:00:00Z".into(),
                    status: ferr_report::FileStatus::Ok,
                    par2_generated: false,
                },
            ],
        }
    }

    #[test]
    fn generate_report_produces_pdf() {
        let dir = std::env::temp_dir().join("ferr_pdf_test");
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("report.pdf");

        generate_report(&sample_manifest(), &output).unwrap();

        assert!(output.exists());
        let size = std::fs::metadata(&output).unwrap().len();
        assert!(size > 0, "Le PDF ne doit pas être vide");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generate_report_many_files() {
        let dir = std::env::temp_dir().join("ferr_pdf_many");
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("report_big.pdf");

        let mut manifest = sample_manifest();
        // Ajouter 60 fichiers pour forcer une deuxième page
        for i in 0..60 {
            manifest.files.push(ferr_report::FileEntry {
                path: format!("clip_{i:03}.braw"),
                size: 512,
                hash_algo: "xxhash64".into(),
                hash: format!("{i:016x}"),
                modified_at: "2025-01-01T00:00:00Z".into(),
                status: ferr_report::FileStatus::Ok,
                par2_generated: false,
            });
        }

        generate_report(&manifest, &output).unwrap();
        assert!(output.exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
