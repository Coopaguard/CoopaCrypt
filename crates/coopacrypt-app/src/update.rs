//! Mise à jour de l'application.
//!
//! Au démarrage, la page demande s'il existe une version plus récente ; si oui,
//! elle propose de la télécharger et de l'installer. Tout se passe ici, côté
//! Rust : la page n'accède ni au réseau ni au disque, elle pose deux questions
//! et affiche les réponses.
//!
//! Le manifeste `latest.json` est publié par le workflow de release sur la
//! dernière release GitHub ; chaque paquet y est signé, et le plugin refuse
//! toute mise à jour dont la signature ne correspond pas à la clé publique
//! embarquée dans `tauri.conf.json`.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Progression du téléchargement, en pourcentage, envoyée à la page.
const EVENT_PROGRESS: &str = "update://progress";

/// Une vérification qui traîne n'a rien à apporter : l'application démarre
/// sans elle, et l'utilisateur ne doit jamais attendre une réponse du réseau.
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// Mise à jour trouvée par [`update_check`], gardée pour [`update_install`].
///
/// Refaire la vérification à l'installation ré-interrogerait le réseau, et
/// pourrait installer une autre version que celle acceptée par l'utilisateur.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

/// Ce que la page a besoin de savoir pour poser la question.
#[derive(Serialize)]
pub struct Available {
    /// Version proposée.
    version: String,
    /// Version en cours d'exécution.
    current: String,
    /// Notes de version, si le manifeste en porte.
    notes: Option<String>,
}

/// Cherche une version plus récente.
///
/// Rend `None` aussi bien s'il n'y en a pas que si la vérification échoue :
/// pas de réseau, serveur injoignable, manifeste illisible… Une mise à jour
/// est une proposition, pas une nécessité — un échec ne mérite qu'une trace
/// sur la sortie d'erreur, jamais une fenêtre.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<Available>, ()> {
    let update = match check(&app).await {
        Ok(update) => update,
        Err(error) => {
            eprintln!("vérification des mises à jour impossible : {error}");
            return Ok(None);
        }
    };

    let Some(update) = update else {
        return Ok(None);
    };

    let available = Available {
        version: update.version.clone(),
        current: update.current_version.clone(),
        notes: update.body.clone(),
    };
    *pending.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(update);
    Ok(Some(available))
}

async fn check(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    app.updater_builder()
        .timeout(CHECK_TIMEOUT)
        .build()?
        .check()
        .await
}

/// Télécharge et installe la mise à jour trouvée par [`update_check`].
///
/// Sur Windows, l'installeur est lancé et le processus courant se termine
/// aussitôt — l'installeur relance l'application lui-même. Ailleurs, le paquet
/// est remplacé en place et l'application est relancée ici.
///
/// La page doit avoir réglé le sort des modifications non enregistrées avant
/// d'appeler : au-delà de ce point, le processus disparaît.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
        .ok_or("aucune mise à jour en attente")?;

    let mut received = 0usize;
    let mut last_percent = u8::MAX;
    update
        .download_and_install(
            |chunk, total| {
                received += chunk;
                let Some(total) = total.filter(|&t| t > 0) else {
                    return;
                };
                let percent = ((received as u64 * 100) / total).min(100) as u8;
                // Un événement par pourcent, pas par paquet reçu.
                if percent != last_percent {
                    last_percent = percent;
                    let _ = app.emit(EVENT_PROGRESS, percent);
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("mise à jour impossible : {e}"))?;

    // Jamais atteint sur Windows : `download_and_install` y termine le
    // processus après avoir lancé l'installeur.
    app.restart();
}
