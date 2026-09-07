use core::fmt;

/// Erreurs du crate.
///
/// Aucune variante ne distingue « mot de passe incorrect » de « fichier altéré » :
/// c'est une propriété voulue du chiffrement authentifié (cf. `FORMAT.md` §4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Le mot de passe est vide. Interdit à l'écriture comme à la lecture.
    EmptyPassword,

    /// Le fichier est trop court ou sa taille ne respecte pas
    /// `(taille - 80) mod 4096 == 0`.
    InvalidLength {
        /// Taille effective du fichier examiné.
        actual: usize,
    },

    /// Les 8 premiers octets ne sont pas `COOCRYPT`. Le fichier est peut-être
    /// au format v1 : voir [`crate::legacy`].
    NotCoocryptFile,

    /// Version de format non reconnue par cette implémentation.
    UnsupportedVersion(u8),

    /// Identifiant de KDF ou d'AEAD non reconnu.
    UnsupportedAlgorithm {
        /// Identifiant de KDF lu dans l'en-tête.
        kdf: u8,
        /// Identifiant d'AEAD lu dans l'en-tête.
        aead: u8,
    },

    /// Un octet réservé de l'en-tête est non nul. Réservé pour de futures
    /// extensions : un v2 strict doit rejeter.
    ReservedBytesNotZero,

    /// Paramètres Argon2id hors des bornes de sûreté, ou impossibles à honorer
    /// sur cette plateforme. Volontairement distinct de [`Error::DecryptionFailed`] :
    /// un utilisateur ne doit jamais croire son mot de passe faux à cause de cela.
    UnsupportedKdfParameters {
        /// Raison du rejet.
        reason: &'static str,
    },

    /// Échec de la vérification du tag d'authentification.
    ///
    /// Mot de passe incorrect **ou** fichier altéré — les deux cas sont
    /// indiscernables par construction.
    DecryptionFailed,

    /// Le champ de longueur du clair déchiffré dépasse la taille disponible.
    MalformedPlaintext,

    /// Le contenu déchiffré n'est pas de l'UTF-8 valide.
    InvalidUtf8,

    /// Le contenu dépasse la capacité du champ de longueur (2^32 - 1 octets).
    ContentTooLarge,

    /// Échec de la dérivation Argon2id.
    KdfFailure,

    /// La source d'entropie du système est indisponible.
    ///
    /// Remontée plutôt qu'ignorée : un sel ou un nonce prévisible ruinerait la
    /// confidentialité en silence.
    RandomSourceUnavailable,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPassword => write!(f, "le mot de passe ne peut pas être vide"),
            Self::InvalidLength { actual } => write!(
                f,
                "taille de fichier invalide ({actual} octets) : ce n'est pas un fichier .coocrypt v2"
            ),
            Self::NotCoocryptFile => write!(f, "signature de fichier absente ou inconnue"),
            Self::UnsupportedVersion(v) => {
                write!(f, "version de format non supportée : {v}")
            }
            Self::UnsupportedAlgorithm { kdf, aead } => write!(
                f,
                "algorithme non supporté (kdf={kdf}, aead={aead})"
            ),
            Self::ReservedBytesNotZero => {
                write!(f, "octets réservés non nuls dans l'en-tête")
            }
            Self::UnsupportedKdfParameters { reason } => {
                write!(f, "paramètres de dérivation non supportés : {reason}")
            }
            Self::DecryptionFailed => {
                write!(f, "mot de passe incorrect ou fichier altéré")
            }
            Self::MalformedPlaintext => write!(f, "contenu déchiffré incohérent"),
            Self::InvalidUtf8 => write!(f, "le contenu déchiffré n'est pas de l'UTF-8 valide"),
            Self::ContentTooLarge => write!(f, "contenu trop volumineux"),
            Self::KdfFailure => write!(f, "échec de la dérivation de clé"),
            Self::RandomSourceUnavailable => {
                write!(f, "source d'entropie système indisponible")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Alias de résultat du crate.
pub type Result<T> = core::result::Result<T, Error>;
