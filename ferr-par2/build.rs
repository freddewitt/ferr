fn main() {
    // Déclarer le cfg custom pour éviter les warnings du compilateur.
    println!("cargo::rustc-check-cfg=cfg(par2_stub)");

    // Le stub peut être forcé via variable d'environnement (tests CI sans par2 installé).
    if std::env::var("FERR_PAR2_STUB").is_ok() {
        println!("cargo:warning=ferr-par2: FERR_PAR2_STUB détecté — stub activé");
        println!("cargo:rustc-cfg=par2_stub");
        return;
    }

    // Vérification souple de la présence du binaire.
    // Si absent à la compilation, la crate compile quand même ;
    // la détection est refaite à l'exécution dans find_par2_binary().
    let found = std::process::Command::new("par2")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|_| true)
        .unwrap_or(false)
        || std::process::Command::new("par2create")
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|_| true)
            .unwrap_or(false);

    if found {
        println!(
            "cargo:warning=ferr-par2: binaire par2 détecté — \
             implémentation subprocess activée"
        );
    } else {
        println!(
            "cargo:warning=ferr-par2: par2 introuvable à la compilation. \
             Installez par2cmdline pour activer la génération PAR2 à l'exécution \
             (brew install par2 / apt install par2 / winget install par2cmdline)."
        );
    }
}
