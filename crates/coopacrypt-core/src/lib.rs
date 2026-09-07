//! Implémentation de référence du format de fichier `.coocrypt`.
//!
//! Spécification : [`FORMAT.md`](https://github.com/Coopaguard/CoopaCrypt/blob/master/FORMAT.md).
//! Ce crate en est la source de vérité ; toute autre implémentation doit
//! reproduire ses vecteurs de test à l'octet près.
//!
//! # Format v2
//!
//! ```text
//! en-tête 64 o (clair, authentifié en AAD) || chiffré (N × 4096) || tag 16 o
//! ```
//!
//! - **Dérivation** : Argon2id v0x13, 256 MiB / t=4 / p=4, sel de 16 octets
//! - **Chiffrement** : XChaCha20-Poly1305, nonce de 24 octets tiré aléatoirement
//! - **Remplissage** : au multiple de 4096 octets, pour masquer la longueur du
//!   contenu à un hébergeur de synchronisation
//!
//! # Exemple
//!
//! ```no_run
//! # fn main() -> Result<(), coopacrypt_core::Error> {
//! let file = coopacrypt_core::encrypt("# Notes\n\nSecret.\n", "phrase de passe")?;
//! let content = coopacrypt_core::decrypt(&file, "phrase de passe")?;
//! assert_eq!(*content, "# Notes\n\nSecret.\n");
//! # Ok(())
//! # }
//! ```
//!
//! # Ce que ce crate ne fait pas
//!
//! Aucune entrée-sortie disque. L'écriture atomique exigée par `FORMAT.md` §4.1
//! (fichier temporaire, `fsync`, remplacement) relève de l'appelant — la CLI et
//! l'application. Ce crate ne manipule que des tampons en mémoire, ce qui le
//! rend testable et portable sans condition.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod header;
mod password;
mod vault;

pub use error::{Error, Result};
pub use header::{KdfParams, BLOCK_LEN, HEADER_LEN, NONCE_LEN, SALT_LEN, TAG_LEN, VERSION};
pub use vault::{decrypt, encrypt, encrypt_with_params, inspect, is_v2, FileInfo, MIN_FILE_LEN};

#[doc(hidden)]
pub use vault::encrypt_deterministic;

/// Prépare un mot de passe comme le fait le format v2 : espaces Unicode
/// ramenés à l'espace ASCII, puis normalisation NFC.
///
/// Exposé pour la génération des vecteurs de test, qui doivent figer cette
/// étape au même titre que le reste.
#[doc(hidden)]
pub fn prepare_password(pass: &str) -> Result<zeroize::Zeroizing<Vec<u8>>> {
    password::prepare(pass)
}
