/// Envoie une notification système "opération terminée".
/// Ne panique jamais — les erreurs de notification sont ignorées silencieusement.
pub fn notify_done(title: &str, message: &str, success: bool) -> anyhow::Result<()> {
    let subtitle = if success {
        "✓ Succès"
    } else {
        "⚠ Terminé avec erreurs"
    };
    send_notification(title, subtitle, message)
}

/// Envoie une notification système d'erreur.
pub fn notify_error(title: &str, error: &str) -> anyhow::Result<()> {
    send_notification(title, "✗ Erreur", error)
}

fn send_notification(title: &str, subtitle: &str, body: &str) -> anyhow::Result<()> {
    let result = notify_rust::Notification::new()
        .appname("ferr")
        .summary(title)
        .subtitle(subtitle)
        .body(body)
        .show();

    // Les erreurs de notification ne sont pas fatales
    if let Err(e) = result {
        eprintln!("ferr-notify : notification non envoyée : {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_done_does_not_panic() {
        // Ne peut pas vérifier que la notification est reçue, mais ne doit pas paniquer
        let _ = notify_done("ferr test", "Copie terminée", true);
    }

    #[test]
    fn notify_error_does_not_panic() {
        let _ = notify_error("ferr test", "Erreur simulée pour test");
    }
}
