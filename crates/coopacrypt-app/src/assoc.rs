//! Association du type de fichier `.coocrypt` avec l'application.
//!
//! Deux mécanismes se complètent, et aucun ne remplace l'autre :
//!
//! - Les **installeurs** (MSI, NSIS, `.deb`, `.rpm`, bundle macOS) déclarent
//!   l'association à l'installation, à partir de `bundle.fileAssociations` dans
//!   `tauri.conf.json`. C'est la voie normale.
//! - Ce module l'enregistre **au démarrage**, sous `HKEY_CURRENT_USER`, pour les
//!   cas que l'installeur ne couvre pas : exécutable copié à la main, clé USB,
//!   version portable, ou installation dont l'association a été écrasée depuis.
//!
//! Windows uniquement. Sur les autres systèmes la fonction ne fait rien : macOS
//! lit l'association dans le `Info.plist` du bundle, et Linux dans le fichier
//! `.desktop`. Ni l'un ni l'autre ne peut être modifié depuis un processus en
//! cours d'exécution.
//!
//! ## Ce que l'enregistrement ne fait pas
//!
//! Écrire sous `HKCU\Software\Classes` rend l'application **disponible** comme
//! gestionnaire. Cela ne la rend gestionnaire **par défaut** que si aucun choix
//! n'a déjà été fait par l'utilisateur : depuis Windows 8, `UserChoice` est
//! protégé par une empreinte et n'est modifiable que depuis les paramètres du
//! système. C'est le comportement souhaitable — un choix explicite de
//! l'utilisateur ne doit pas être écrasé par une application qui démarre.

/// Résultat d'une tentative d'enregistrement.
///
/// Hors Windows, seule `Skipped` est construite : les deux autres variantes
/// déclencheraient un `dead_code`, promu en erreur par le `-D warnings` de
/// l'intégration continue. Les décrire quand même garde une seule forme de
/// retour pour toutes les plateformes.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Les clés ont été écrites.
    Registered,
    /// L'association pointait déjà vers cet exécutable : rien n'a été touché.
    AlreadyCurrent,
    /// Plateforme sans registre, ou compilation de développement.
    Skipped,
}

#[cfg(windows)]
mod imp {
    use std::io;
    use std::path::Path;

    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    use super::Outcome;

    /// Identifiant de type de fichier. Il sert de clé dans le registre et ne
    /// doit pas changer : le modifier laisserait l'ancienne entrée orpheline.
    const PROG_ID: &str = "CoopaCrypt.Vault";
    const EXT: &str = ".coocrypt";
    const MIME: &str = "application/x-coocrypt";
    /// Libellé affiché dans la colonne « Type » de l'explorateur.
    const LABEL: &str = "Coffre CoopaCrypt";

    const CLASSES: &str = r"Software\Classes";

    pub fn ensure_registered(exe: &Path) -> io::Result<Outcome> {
        let exe = exe.to_string_lossy();
        let command = format!("\"{exe}\" \"%1\"");
        let icon = format!("\"{exe}\",0");

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let command_path = format!(r"{CLASSES}\{PROG_ID}\shell\open\command");

        // Sans ce court-circuit, chaque démarrage réécrirait les mêmes valeurs
        // et notifierait l'explorateur pour rien.
        if let Ok(key) = hkcu.open_subkey(&command_path) {
            if key.get_value::<String, _>("").ok().as_deref() == Some(command.as_str()) {
                return Ok(Outcome::AlreadyCurrent);
            }
        }

        // Le type de fichier lui-même.
        let (prog, _) = hkcu.create_subkey(format!(r"{CLASSES}\{PROG_ID}"))?;
        prog.set_value("", &LABEL)?;

        let (icon_key, _) = hkcu.create_subkey(format!(r"{CLASSES}\{PROG_ID}\DefaultIcon"))?;
        icon_key.set_value("", &icon)?;

        let (cmd_key, _) = hkcu.create_subkey(&command_path)?;
        cmd_key.set_value("", &command)?;

        // L'extension, rattachée au type.
        let (ext, _) = hkcu.create_subkey(format!(r"{CLASSES}\{EXT}"))?;
        ext.set_value("", &PROG_ID)?;
        ext.set_value("Content Type", &MIME)?;

        // Déclaration sous `Applications` : elle fait apparaître CoopaCrypt
        // dans « Ouvrir avec » même lorsque `UserChoice` désigne une autre
        // application, ce que l'enregistrement ci-dessus ne permet pas.
        if let Some(name) = Path::new(exe.as_ref()).file_name() {
            let app = format!(r"{CLASSES}\Applications\{}", name.to_string_lossy());

            let (app_cmd, _) = hkcu.create_subkey(format!(r"{app}\shell\open\command"))?;
            app_cmd.set_value("", &command)?;

            let (supported, _) = hkcu.create_subkey(format!(r"{app}\SupportedTypes"))?;
            // La présence du nom suffit ; la valeur n'est pas lue.
            supported.set_value(EXT, &"")?;
        }

        notify_shell();
        Ok(Outcome::Registered)
    }

    /// Prévient l'explorateur que les associations ont changé.
    ///
    /// Sans cet appel, les icônes et le menu contextuel restent ceux mis en
    /// cache jusqu'à la prochaine ouverture de session.
    fn notify_shell() {
        use windows_sys::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

        // Les deux éléments sont nuls : la notification porte sur l'ensemble
        // des associations, pas sur un chemin particulier.
        unsafe {
            SHChangeNotify(
                SHCNE_ASSOCCHANGED as i32,
                SHCNF_IDLIST,
                std::ptr::null(),
                std::ptr::null(),
            );
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::io;
    use std::path::Path;

    use super::Outcome;

    pub fn ensure_registered(_exe: &Path) -> io::Result<Outcome> {
        Ok(Outcome::Skipped)
    }
}

/// Enregistre l'association `.coocrypt` pour l'utilisateur courant.
///
/// L'échec n'est jamais fatal : une association manquante est une gêne, pas une
/// raison d'empêcher l'application de démarrer.
///
/// En compilation de développement, l'enregistrement est **ignoré** : il
/// pointerait vers `target/debug`, et un `cargo clean` laisserait une
/// association cassée. Poser `COOPACRYPT_REGISTER_ASSOC=1` force le traitement,
/// pour pouvoir l'éprouver sans passer par un build de production.
pub fn ensure_registered() -> std::io::Result<Outcome> {
    if cfg!(debug_assertions) && std::env::var_os("COOPACRYPT_REGISTER_ASSOC").is_none() {
        return Ok(Outcome::Skipped);
    }

    let exe = std::env::current_exe()?;
    imp::ensure_registered(&exe)
}
