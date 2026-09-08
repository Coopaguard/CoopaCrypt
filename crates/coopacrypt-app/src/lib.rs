//! Backend de l'application de bureau CoopaCrypt.
//!
//! La page n'a **aucun accès** au chiffrement ni au système de fichiers : elle
//! passe par les commandes de ce module. Le mot de passe reste côté Rust
//! (cf. [`session`]), et les erreurs remontées ne distinguent jamais un mauvais
//! mot de passe d'un fichier altéré.

// `windows_subsystem` est posé dans `main.rs` : ici il serait sans effet, la
// bibliothèque n'étant pas la racine du crate lié en exécutable.

mod assoc;
mod atomic;
mod launch;
mod session;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{Emitter, Manager, State};
use zeroize::Zeroizing;

use launch::Pending;
use session::Session;

/// Signale à la page qu'un coffre attend d'être ouvert.
///
/// L'événement ne transporte **pas** le chemin : il invite seulement la page à
/// venir le réclamer par [`pending_vault`]. Faire passer le chemin par
/// l'événement le dupliquerait, avec le risque d'ouvrir deux fois le même
/// fichier ; la lecture, elle, consomme la valeur.
const EVENT_PENDING: &str = "vault://pending";

/// Exécute un traitement coûteux hors du thread de l'interface.
///
/// Une commande Tauri **synchrone s'exécute sur le thread principal**. Or une
/// dérivation Argon2id à 256 MiB prend près d'une seconde : la fenêtre gèlerait
/// à chaque ouverture et à chaque enregistrement, avec les artefacts
/// d'affichage qui accompagnent une boucle d'événements bloquée — curseur
/// compris.
///
/// Les commandes coûteuses sont donc `async` et confient le calcul à un thread
/// dédié.
async fn offload<T, F>(work: F) -> CmdResult<T>
where
    F: FnOnce() -> CmdResult<T> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| "le traitement a été interrompu".to_owned())?
}

/// Message d'erreur destiné à la page.
///
/// Les erreurs du cœur sont rendues telles quelles : elles sont déjà rédigées
/// pour un utilisateur et prennent soin de ne pas distinguer « mot de passe
/// incorrect » de « fichier altéré ».
type CmdResult<T> = Result<T, String>;

/// État de session tel que la page peut l'afficher.
#[derive(Serialize)]
pub struct SessionState {
    unlocked: bool,
    /// Chemin du coffre ouvert. Un chemin n'est pas du contenu de coffre.
    path: Option<String>,
    /// Secondes restantes avant verrouillage automatique.
    remaining_secs: Option<u64>,
}

/// En-tête d'un coffre, lisible sans mot de passe.
#[derive(Serialize)]
pub struct VaultInfo {
    version: u8,
    memory_kib: u32,
    iterations: u32,
    parallelism: u8,
    file_len: usize,
    blocks: usize,
    /// Vrai si les paramètres sont ceux de la version courante.
    up_to_date: bool,
}

/// Ouvre un coffre et rend son contenu déchiffré.
#[tauri::command]
async fn vault_open(
    path: String,
    password: String,
    session: State<'_, Session>,
) -> CmdResult<String> {
    let password = Zeroizing::new(password);
    let path = PathBuf::from(path);

    let content = {
        let path = path.clone();
        let password = password.clone();
        offload(move || {
            let bytes = std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}"))?;

            // Rejet avant toute dérivation : inutile de faire patienter une
            // seconde l'utilisateur pour un fichier qui n'est pas un coffre.
            if !coopacrypt_core::is_v2(&bytes) {
                return Err("ce fichier n'est pas un coffre .coocrypt".into());
            }

            coopacrypt_core::decrypt(&bytes, &password)
                .map(|content| content.to_string())
                .map_err(|e| e.to_string())
        })
        .await?
    };

    session.unlock(path, password);
    Ok(content)
}

/// Crée un coffre vide et ouvre la session dessus.
#[tauri::command]
async fn vault_create(
    path: String,
    password: String,
    session: State<'_, Session>,
) -> CmdResult<()> {
    let password = Zeroizing::new(password);
    let path = PathBuf::from(path);

    // Un coffre n'a pas de sauvegarde : ne jamais écraser sans intention claire.
    if path.exists() {
        return Err("ce fichier existe déjà".into());
    }

    {
        let path = path.clone();
        let password = password.clone();
        offload(move || {
            let bytes = coopacrypt_core::encrypt("", &password).map_err(|e| e.to_string())?;
            atomic::write(&path, &bytes).map_err(|e| format!("écriture impossible : {e}"))
        })
        .await?;
    }

    session.unlock(path, password);
    Ok(())
}

/// Enregistre le document dans le coffre ouvert.
///
/// Le contenu est écrit **tel quel**, octet pour octet : aucune normalisation
/// Unicode, aucune conversion de fins de ligne (`FORMAT.md` §2.1.3).
#[tauri::command]
async fn vault_save(content: String, session: State<'_, Session>) -> CmdResult<()> {
    let (path, password) = session.credentials().ok_or("session verrouillée")?;

    offload(move || {
        // Les paramètres par défaut de la version courante sont réappliqués :
        // chaque enregistrement durcit silencieusement le fichier.
        let bytes = coopacrypt_core::encrypt(&content, &password).map_err(|e| e.to_string())?;
        atomic::write(&path, &bytes).map_err(|e| format!("écriture impossible : {e}"))
    })
    .await
}

/// Enregistre sous un autre chemin et bascule la session dessus.
#[tauri::command]
async fn vault_save_as(
    path: String,
    content: String,
    password: String,
    session: State<'_, Session>,
) -> CmdResult<()> {
    let password = Zeroizing::new(password);
    let path = PathBuf::from(path);

    {
        let path = path.clone();
        let password = password.clone();
        offload(move || {
            let bytes = coopacrypt_core::encrypt(&content, &password).map_err(|e| e.to_string())?;
            atomic::write(&path, &bytes).map_err(|e| format!("écriture impossible : {e}"))
        })
        .await?;
    }

    session.unlock(path, password);
    Ok(())
}

/// Change le mot de passe du coffre ouvert, par re-chiffrement complet.
#[tauri::command]
async fn vault_change_password(
    content: String,
    new_password: String,
    session: State<'_, Session>,
) -> CmdResult<()> {
    let new_password = Zeroizing::new(new_password);
    let path = session.path().ok_or("session verrouillée")?;

    {
        let path = path.clone();
        let password = new_password.clone();
        offload(move || {
            let bytes = coopacrypt_core::encrypt(&content, &password).map_err(|e| e.to_string())?;
            atomic::write(&path, &bytes).map_err(|e| format!("écriture impossible : {e}"))
        })
        .await?;
    }

    session.unlock(path, new_password);
    Ok(())
}

/// Lit l'en-tête d'un coffre sans mot de passe.
#[tauri::command]
async fn vault_info(path: String) -> CmdResult<VaultInfo> {
    let bytes =
        offload(move || std::fs::read(&path).map_err(|e| format!("lecture impossible : {e}")))
            .await?;
    let info = coopacrypt_core::inspect(&bytes).map_err(|e| e.to_string())?;
    Ok(VaultInfo {
        version: info.version,
        memory_kib: info.kdf.memory_kib,
        iterations: info.kdf.iterations,
        parallelism: info.kdf.parallelism,
        file_len: info.file_len,
        blocks: info.blocks,
        up_to_date: info.kdf == coopacrypt_core::KdfParams::DEFAULT,
    })
}

/// Verrouille la session et efface le mot de passe.
#[tauri::command]
fn vault_lock(session: State<'_, Session>) {
    session.lock();
}

/// Rend l'état de session, en appliquant le délai d'inactivité.
#[tauri::command]
fn session_state(session: State<'_, Session>) -> SessionState {
    let unlocked = session.is_unlocked();
    SessionState {
        unlocked,
        path: session.path().map(|p| p.to_string_lossy().into_owned()),
        remaining_secs: session.remaining_secs(),
    }
}

/// Signale une activité de l'utilisateur et repousse l'échéance.
#[tauri::command]
fn session_touch(session: State<'_, Session>) {
    session.touch();
}

/// Rend le coffre demandé au lancement, et l'oublie.
///
/// La page l'appelle au chargement, puis à chaque réception de
/// [`EVENT_PENDING`]. Rendre `None` est le cas courant : l'application démarrée
/// depuis un raccourci n'a pas de coffre à ouvrir.
#[tauri::command]
fn pending_vault(pending: State<'_, Pending>) -> Option<String> {
    pending
        .take()
        .map(|path| path.to_string_lossy().into_owned())
}

/// Point d'entrée de l'application.
///
/// # Panics
///
/// Si l'environnement graphique ne permet pas de démarrer Tauri.
pub fn run() {
    tauri::Builder::default()
        // À déclarer en premier : le plugin doit trancher entre les instances
        // avant que quoi que ce soit d'autre ne s'initialise.
        //
        // Un double-clic sur un coffre lance systématiquement un nouveau
        // processus. Sans cette écluse, deux fenêtres pourraient éditer le même
        // fichier, et le dernier enregistrement écraserait silencieusement le
        // travail de l'autre — l'écriture est atomique, pas concurrente.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = launch::from_args(argv) {
                app.state::<Pending>().set(path);
                let _ = app.emit(EVENT_PENDING, ());
            }

            // La seconde instance s'arrête : sans reprise de focus, le
            // double-clic n'aurait aucun effet visible.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(Session::default());

            let pending = Pending::default();
            // Windows et Linux transmettent le chemin en argument. Sur macOS
            // il arrivera plus tard, par `RunEvent::Opened`.
            if let Some(path) = launch::from_args(std::env::args()) {
                pending.set(path);
            }
            app.manage(pending);

            // Une association manquante est une gêne, pas une raison
            // d'empêcher le démarrage : l'échec est seulement tracé.
            match assoc::ensure_registered() {
                Ok(assoc::Outcome::Registered) => {
                    eprintln!("association .coocrypt enregistrée");
                }
                Ok(assoc::Outcome::AlreadyCurrent | assoc::Outcome::Skipped) => {}
                Err(error) => eprintln!("association .coocrypt non enregistrée : {error}"),
            }

            // L'icône de la barre des tâches est celle de la **fenêtre**, pas
            // celle du fichier exécutable. La définir explicitement évite de
            // dépendre de la ressource Windows embarquée à la compilation, qui
            // n'est présente que sur certains profils de build.
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
                window.set_icon(icon)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            vault_open,
            vault_create,
            vault_save,
            vault_save_as,
            vault_change_password,
            vault_info,
            vault_lock,
            session_state,
            session_touch,
            pending_vault,
        ])
        .build(tauri::generate_context!())
        .expect("démarrage de l'application")
        .run(|app, event| on_run_event(app, &event));
}

/// Traite les événements du cycle de vie de l'application.
///
/// Seules les plateformes Apple en produisent un qui nous concerne : macOS et
/// iOS ne passent pas le fichier ouvert en argument de ligne de commande mais
/// émettent [`tauri::RunEvent::Opened`], y compris pour un « Ouvrir avec »
/// reçu alors que l'application tourne déjà.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn on_run_event(app: &tauri::AppHandle, event: &tauri::RunEvent) {
    let tauri::RunEvent::Opened { urls } = event else {
        return;
    };

    // Les URL sont normalement en `file://`. Le repli sur la chaîne brute
    // couvre le cas où la conversion échoue, plutôt que d'ignorer la demande.
    let first = urls
        .iter()
        .find_map(|url| url.to_file_path().ok())
        .or_else(|| urls.first().map(|url| PathBuf::from(url.as_str())));

    if let Some(path) = first {
        app.state::<Pending>().set(path);
        // Perdu si la page n'est pas encore chargée : elle interrogera
        // `pending_vault` de toute façon à son démarrage.
        let _ = app.emit(EVENT_PENDING, ());
    }
}

/// Aucun événement à traiter hors des plateformes Apple : le chemin y arrive
/// en argument de ligne de commande, lu au démarrage.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn on_run_event(_app: &tauri::AppHandle, _event: &tauri::RunEvent) {}
