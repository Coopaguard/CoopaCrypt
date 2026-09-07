//! Coffre demandé au lancement, par double-clic ou « Ouvrir avec ».
//!
//! Le chemin arrive par deux voies distinctes selon le système :
//!
//! - **Windows et Linux** : en argument de ligne de commande. L'association
//!   enregistrée exécute `"coopacrypt-app.exe" "%1"`.
//! - **macOS** : par l'événement `RunEvent::Opened`, jamais par `argv`.
//!
//! Dans les deux cas le chemin est déposé ici plutôt qu'émis vers la page :
//! `RunEvent::Opened` se déclenche **avant** que la page soit chargée, et un
//! événement Tauri émis sans auditeur est perdu. La page vient donc le
//! réclamer quand elle est prête.

use std::path::PathBuf;
use std::sync::Mutex;

/// Chemin en attente de prise en charge par la page.
///
/// La valeur est **consommée** à la lecture : sans cela, un rechargement de la
/// page rouvrirait le fichier du lancement précédent.
#[derive(Default)]
pub struct Pending(Mutex<Option<PathBuf>>);

impl Pending {
    /// Dépose un chemin, en remplaçant celui qui attendait éventuellement.
    ///
    /// Le remplacement est volontaire : si l'utilisateur double-clique deux
    /// coffres coup sur coup avant que la page ne réagisse, c'est le dernier
    /// qui exprime son intention.
    pub fn set(&self, path: PathBuf) {
        // Un `Mutex` empoisonné ne peut venir que d'une panique alors qu'il
        // était tenu, ce qui n'arrive pas ici : seul un `Option` est manipulé.
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(path);
        }
    }

    /// Retire et rend le chemin en attente, s'il y en a un.
    pub fn take(&self) -> Option<PathBuf> {
        self.0.lock().ok().and_then(|mut slot| slot.take())
    }
}

/// Extrait un chemin de coffre des arguments de lancement.
///
/// Le premier argument est le programme lui-même, et tout ce qui commence par
/// `-` est une option — WebView2 et les environnements de bureau en ajoutent
/// parfois. Le premier argument restant est retenu.
///
/// Aucune vérification d'extension : un coffre peut être nommé librement, et
/// c'est l'ouverture qui tranche, le cœur rejetant tout fichier qui n'est pas un
/// coffre. Filtrer sur `.coocrypt` ici ne ferait qu'échouer plus tôt et pour une
/// mauvaise raison.
pub fn from_args<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = String>,
{
    args.into_iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-') && !arg.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn le_chemin_est_pris_apres_le_nom_du_programme() {
        let found = from_args(args(&["coopacrypt-app.exe", r"C:\coffres\perso.coocrypt"]));
        assert_eq!(found, Some(PathBuf::from(r"C:\coffres\perso.coocrypt")));
    }

    #[test]
    fn sans_argument_il_n_y_a_rien_a_ouvrir() {
        assert_eq!(from_args(args(&["coopacrypt-app.exe"])), None);
    }

    /// Le nom du programme ressemble à un chemin : le sauter n'est pas
    /// facultatif, sans quoi l'application tenterait de s'ouvrir elle-même.
    #[test]
    fn le_nom_du_programme_n_est_jamais_retenu() {
        assert_eq!(from_args(args(&[r"C:\Program Files\app.exe"])), None);
    }

    #[test]
    fn les_options_sont_ignorees() {
        let found = from_args(args(&[
            "coopacrypt-app",
            "--no-sandbox",
            "-v",
            "/home/x/perso.coocrypt",
        ]));
        assert_eq!(found, Some(PathBuf::from("/home/x/perso.coocrypt")));
    }

    /// Un environnement de bureau peut transmettre un argument vide ; le
    /// convertir en chemin donnerait une erreur d'ouverture incompréhensible.
    #[test]
    fn un_argument_vide_n_est_pas_un_chemin() {
        let found = from_args(args(&["app", "", "vrai.coocrypt"]));
        assert_eq!(found, Some(PathBuf::from("vrai.coocrypt")));
    }

    #[test]
    fn un_fichier_sans_extension_reste_recevable() {
        // C'est l'ouverture qui rejette un fichier qui n'est pas un coffre.
        let found = from_args(args(&["app", "/tmp/notes"]));
        assert_eq!(found, Some(PathBuf::from("/tmp/notes")));
    }

    #[test]
    fn la_lecture_consomme_le_chemin_en_attente() {
        let pending = Pending::default();
        assert_eq!(pending.take(), None);

        pending.set(PathBuf::from("a.coocrypt"));
        assert_eq!(pending.take(), Some(PathBuf::from("a.coocrypt")));
        // Un rechargement de la page ne doit pas rouvrir le même fichier.
        assert_eq!(pending.take(), None);
    }

    #[test]
    fn le_dernier_chemin_depose_remplace_le_precedent() {
        let pending = Pending::default();
        pending.set(PathBuf::from("a.coocrypt"));
        pending.set(PathBuf::from("b.coocrypt"));
        assert_eq!(pending.take(), Some(PathBuf::from("b.coocrypt")));
    }
}
