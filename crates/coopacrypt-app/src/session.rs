//! État de session : le coffre ouvert et son mot de passe.
//!
//! # Pourquoi le mot de passe vit ici, et non dans la page
//!
//! Chaque enregistrement tire un nouveau sel et impose donc une nouvelle
//! dérivation, qui exige le mot de passe en clair. Il faut bien le conserver
//! pendant la session.
//!
//! Le garder côté Rust plutôt que dans le contexte JavaScript le met hors de
//! portée du DOM, des outils de développement et de toute dépendance front.
//! La page ne l'envoie qu'une fois, au déverrouillage, et ne le revoit jamais.
//!
//! Le contenu déchiffré, lui, réside forcément dans la page : c'est ce que
//! l'éditeur affiche. Cette asymétrie est assumée.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use zeroize::Zeroizing;

/// Délai d'inactivité au-delà duquel la session se verrouille.
///
/// Appliqué **côté Rust**. Un minuteur dans la page serait un confort, pas une
/// garantie : rien n'assure qu'il s'exécute si la vue est figée ou suspendue.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Coffre déverrouillé.
struct Unlocked {
    path: PathBuf,
    password: Zeroizing<String>,
    last_activity: Instant,
}

/// Session de l'application, partagée par toutes les commandes.
#[derive(Default)]
pub struct Session {
    inner: Mutex<Option<Unlocked>>,
}

impl Session {
    /// Déclare un coffre ouvert et mémorise son mot de passe.
    pub fn unlock(&self, path: PathBuf, password: Zeroizing<String>) {
        let mut guard = self.inner.lock().expect("session empoisonnée");
        *guard = Some(Unlocked {
            path,
            password,
            last_activity: Instant::now(),
        });
    }

    /// Verrouille la session et efface le mot de passe de la mémoire.
    pub fn lock(&self) {
        let mut guard = self.inner.lock().expect("session empoisonnée");
        // `Zeroizing` efface le tampon à la destruction.
        *guard = None;
    }

    /// Chemin du coffre ouvert, sans prolonger la session.
    pub fn path(&self) -> Option<PathBuf> {
        let guard = self.inner.lock().expect("session empoisonnée");
        guard.as_ref().map(|u| u.path.clone())
    }

    /// Indique si la session est déverrouillée, après application du délai.
    pub fn is_unlocked(&self) -> bool {
        self.enforce_idle_timeout();
        self.inner.lock().expect("session empoisonnée").is_some()
    }

    /// Secondes restantes avant verrouillage automatique.
    pub fn remaining_secs(&self) -> Option<u64> {
        self.enforce_idle_timeout();
        let guard = self.inner.lock().expect("session empoisonnée");
        guard.as_ref().map(|u| {
            IDLE_TIMEOUT
                .saturating_sub(u.last_activity.elapsed())
                .as_secs()
        })
    }

    /// Repousse l'échéance d'inactivité.
    pub fn touch(&self) {
        let mut guard = self.inner.lock().expect("session empoisonnée");
        if let Some(unlocked) = guard.as_mut() {
            unlocked.last_activity = Instant::now();
        }
    }

    /// Prend une copie du chemin et du mot de passe, en repoussant l'échéance.
    ///
    /// Une copie est nécessaire parce que le chiffrement est déplacé sur un
    /// autre thread : on ne peut pas y tenir le verrou de la session. La copie
    /// du mot de passe reste enveloppée dans `Zeroizing` et s'efface donc dès
    /// que l'opération se termine.
    ///
    /// Rend `None` si la session est verrouillée — y compris si le délai vient
    /// d'expirer, vérifié au même instant.
    pub fn credentials(&self) -> Option<(PathBuf, Zeroizing<String>)> {
        self.enforce_idle_timeout();
        let mut guard = self.inner.lock().expect("session empoisonnée");
        let unlocked = guard.as_mut()?;
        unlocked.last_activity = Instant::now();
        Some((
            unlocked.path.clone(),
            Zeroizing::new(unlocked.password.to_string()),
        ))
    }

    /// Verrouille si le délai d'inactivité est dépassé.
    fn enforce_idle_timeout(&self) {
        let mut guard = self.inner.lock().expect("session empoisonnée");
        if let Some(unlocked) = guard.as_ref() {
            if unlocked.last_activity.elapsed() >= IDLE_TIMEOUT {
                *guard = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_ouverte() -> Session {
        let session = Session::default();
        session.unlock(
            PathBuf::from("coffre.coocrypt"),
            Zeroizing::new("phrase de passe".to_owned()),
        );
        session
    }

    #[test]
    fn une_session_neuve_est_verrouillee() {
        let session = Session::default();
        assert!(!session.is_unlocked());
        assert!(session.path().is_none());
        assert!(session.credentials().is_none());
    }

    #[test]
    fn le_deverrouillage_expose_les_identifiants() {
        let session = session_ouverte();
        assert!(session.is_unlocked());
        let (path, password) = session.credentials().expect("session déverrouillée");
        assert_eq!(path, PathBuf::from("coffre.coocrypt"));
        assert_eq!(*password, "phrase de passe");
    }

    #[test]
    fn le_verrouillage_coupe_tout_acces() {
        let session = session_ouverte();
        session.lock();
        assert!(!session.is_unlocked());
        assert!(session.path().is_none());
        assert!(session.credentials().is_none());
    }

    #[test]
    fn l_echeance_est_repoussee_par_l_activite() {
        let session = session_ouverte();
        let avant = session.remaining_secs().unwrap();
        session.touch();
        assert!(session.remaining_secs().unwrap() >= avant);
    }

    #[test]
    fn une_echeance_depassee_verrouille() {
        let session = session_ouverte();
        {
            // Simule une inactivité prolongée sans attendre réellement.
            let mut guard = session.inner.lock().unwrap();
            guard.as_mut().unwrap().last_activity = Instant::now() - IDLE_TIMEOUT;
        }
        assert!(!session.is_unlocked(), "le délai n'a pas été appliqué");
        assert!(session.credentials().is_none());
    }
}
