# Plan de Développement : ferr-app (Interface Iced)

## 1. AUDIT DE L'EXISTANT
Après analyse approfondie des crates backend qui composent l'écosystème **ferr**, voici les API publiques que `ferr-app` consommera :
- **ferr-core** : `run_copy`, `run_watch`, `check_space`, fonctions de gestion de profils (`load_profile`, `save_profile`, etc.).
- **ferr-verify** : `verify_dirs`, `verify_manifest`, `scan_bitrot`.
- **ferr-session** : `list_sessions`, `get_session`, `find_file_by_hash`.
- **ferr-camera** : `scan_clips`, `verify_clip_integrity`, `apply_rename_template`.
- **ferr-report/par2** : `export_ale`, `export_csv`, `ferr_par2::repair`.

**Conclusion Formelle** : **Aucune modification des crates existantes n’est nécessaire.**
L'architecture existante de `ferr` est déjà agnostique de l'interface et ses fonctions `pub` (tout comme les types `CopyJob` et sa signature parallèle `Send + Sync + Clone`) sont parfaitement compatibles avec les contraintes asynchrones d'Iced. `ferr-app` viendra se brancher par-dessus de la même façon que `ferr-cli`.

---

## 2. ARCHITECTURE DE FERR-APP

Le framework **Iced** impose une architecture unidirectionnelle inspirée d'Elm.
- **`state/`** : Stocke la donnée UI. Composé du constructeur général `AppState` et de l'état local respectif à chaque onglet (`CopyState`, `WatchState`...).
- **`ui/`** : Composants purement visuels (`Element<Message>`) sans aucune logique d'affaires. Ils réagissent aux modifications du `state`.
- **`bridge/`** : Le pont asynchrone non-bloquant. Ces modules encapsulent les lourdes boucles d’affaires (ex. `ferr_core::run_copy`) au sein de sous-processus systèmes `std::thread::spawn` isolés, reliés à Iced via des `subscription::channel`. L'UI tourne constamment à 60 FPS car le backend rust travaille en toile de fond via `tokio` multi-threading.

---

## 3. ARBORESCENCE COMPLÈTE

```text
ferr-app/
├── Cargo.toml
└── src/
    ├── main.rs                 # Initialisation système Iced + configuration de la fenêtre
    ├── app.rs                  # Moteur Elm global, Sidebar, Titlebar, Routing
    ├── theme.rs                # Constantes de couleurs et de typographie strictes
    │
    ├── state/                  # Logique d'état des pages
    │   ├── mod.rs
    │   ├── app_state.rs
    │   ├── copy_state.rs
    │   ├── watch_state.rs
    │   ├── verify_state.rs
    │   ├── history_state.rs
    │   ├── profile_state.rs
    │   ├── scan_state.rs
    │   └── camera_state.rs
    │
    ├── bridge/                 # Enveloppes asynchrones (Iced Subscriptions)
    │   ├── mod.rs
    │   ├── copy_bridge.rs
    │   ├── verify_bridge.rs
    │   ├── watch_bridge.rs
    │   ├── scan_bridge.rs
    │   └── profile_bridge.rs
    │
    └── ui/                     # Widgets isolés
        ├── mod.rs
        ├── components/
        │   ├── mod.rs
        │   ├── toggle.rs
        │   ├── drop_zone.rs
        │   ├── dest_card.rs
        │   ├── progress_bar.rs
        │   ├── stat_card.rs
        │   └── par2_panel.rs
        └── tabs/
            ├── mod.rs
            ├── copy_tab.rs
            ├── watch_tab.rs
            ├── verify_tab.rs
            ├── history_tab.rs
            ├── profiles_tab.rs
            ├── scan_tab.rs
            └── camera_tab.rs
```

---

## 4. DÉPENDANCES ET VERSIONS

| Dépendance | Version | Rôle | Accessibilité |
|------------|---------|------|---------------|
| `ferr-core`, `ferr-report` | `{ path = "..." }` | Interfaçage interne métier | Locale |
| `iced` | `0.13` | Moteur GUI performant. Features : `tokio`, `image`, `svg` (pour SVG natifs) | Crates.io |
| `tokio` | `1.0` | Exécuteur asynchrone indispensable pour `iced::subscription` | Crates.io |
| `rfd` | `0.14` | Connecteur natif pour invoquer la boîte de dialogue d'exploration de fichiers de macOS Finder | Crates.io |
| `serde` & `serde_json` | `1.0` | Formatage et sérialisation des états persistants et profils | Crates.io |
| `chrono` | `0.4` | Gestion du temps UI | Crates.io |
| `dirs` | `5.0` | Récupération multiplateforme du `~/.config/ferr` | Crates.io |

Toutes ces caisses sont légères, matures et éprouvées sur l'écosystème Rust.

---

## 5. ORDRE DE CONSTRUCTION ÉTAPE PAR ÉTAPE

1. **Socle Bas Niveau** :
   - Mise en place du crate de base dans le `Cargo.toml` et du `main.rs` racine (hello world Iced).
   - *Validation :* Compiler `ferr-app` génère une fenêtre vide sans bloquer.
2. **Design System (`theme.rs`, `app.rs`)** :
   - Définition stricte des palettes, variables de typographie, de la Sidebar, de la NavBar et du Titlebar asymétrique macOS.
   - *Validation :* UI élégante et navigation réactive (sans état métier).
3. **Logique Métier Isolée (`bridge/`)** :
   - Implémentation des abonnements asynchrones (`CopyBridge`, `WatchBridge`).
   - *Validation :* Connexion établie avec l'API `ferr-core`.
4. **Composants d'UI Pures (`ui/components/`)** :
   - Assemblage mathématique de `drop_zone`, `toggle`, `dest_card` qui exigent des clics.
   - *Validation :* Réutilisabilité testée visuellement.
5. **Assemblage Métier (`ui/tabs/` + `state/`)** :
   - Montage de chaque onglet et de la structure de message pour inter-communiquer avec l'utilisateur et le Bridge en temps réel.
   - *Validation Finale* : Flux de copie simulé graphiquement bout-en-bout.

---

## 6. RISQUES IDENTIFIÉS ET MODÈLES D'ADAPTATION

| Risque | Description | Solution Assurée |
|--------|-------------|------------------|
| **Iced bloqué (Freeze UI)** | Une opération de calcul crypto ou IO ralentit les 60 fps du GUI Iced. | **Tokio Channels**. Le processus de fond est sur un thread de l'OS (`std::thread::spawn`), et ne transmet qu'un minuscule objet "Message Iced" en non-bloquant `blocking_send()` à un channel `tokio` asynchrone géré par Iced. Les FPS sont garantis. |
| **Surcharge des notifications (Flood UI)** | Les callbacks de progression de copie très rapides innondent le canal de message (ex: 100 x/sec) poussant Iced à redessiner frénétiquement. | Le trait `run_copy` possède déjà nativement dans `ferr-core` une horloge de rafraichissement intégrée. |
| **Erreurs fatales & Fermetures inopinées** | Une fonction "unwrap" dans le cycle d'UI ferait crasher violemment la fenêtre. | **Politique 0 `unwrap` dans le thread Iced**. `ferr-app` affichera toujours l'erreur dans l'UI via les variantes `ErrorMessage(String)`. |
