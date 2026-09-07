//! En-tête du format v2 (`FORMAT.md` §3.2).
//!
//! 64 octets en clair, authentifiés intégralement comme données associées (AAD).
//! Tous les entiers sont en big-endian.
//!
//! ```text
//! Offset  Taille  Champ
//!      0       8  Magic "COOCRYPT"
//!      8       1  Version du format (2)
//!      9       1  Identifiant KDF (1 = Argon2id)
//!     10       1  Identifiant AEAD (1 = XChaCha20-Poly1305)
//!     11       1  Réservé (0)
//!     12       4  Argon2id — coût mémoire en KiB (u32)
//!     16       4  Argon2id — itérations (u32)
//!     20       1  Argon2id — parallélisme (u8)
//!     21       3  Réservé (0 0 0)
//!     24      16  Sel
//!     40      24  Nonce
//! ```

use crate::error::{Error, Result};

/// Signature de fichier du format v2.
pub const MAGIC: [u8; 8] = *b"COOCRYPT";
/// Numéro de version du format décrit par ce module.
pub const VERSION: u8 = 2;
/// Identifiant de KDF : Argon2id.
pub const KDF_ARGON2ID: u8 = 1;
/// Identifiant d'AEAD : XChaCha20-Poly1305.
pub const AEAD_XCHACHA20POLY1305: u8 = 1;

/// Taille de l'en-tête en clair, en octets.
pub const HEADER_LEN: usize = 64;
/// Taille du sel Argon2id, en octets.
pub const SALT_LEN: usize = 16;
/// Taille du nonce XChaCha20, en octets.
pub const NONCE_LEN: usize = 24;
/// Taille du tag Poly1305, en octets.
pub const TAG_LEN: usize = 16;
/// Taille de la clé dérivée, en octets.
pub const KEY_LEN: usize = 32;

/// Taille de bloc du remplissage (`FORMAT.md` §2.3).
pub const BLOCK_LEN: usize = 4096;

/// Taille du préfixe de longueur du clair interne.
pub const LEN_PREFIX: usize = 4;

/// Paramètres de dérivation Argon2id portés par l'en-tête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Coût mémoire, en KiB.
    pub memory_kib: u32,
    /// Nombre d'itérations (coût temps).
    pub iterations: u32,
    /// Degré de parallélisme (lanes).
    pub parallelism: u8,
}

impl KdfParams {
    /// Paramètres par défaut de cette version : 128 MiB, t=4, p=4.
    ///
    /// Utilisés à **chaque** écriture, y compris lors de la réécriture d'un
    /// fichier créé avec des paramètres plus faibles : le durcissement est
    /// automatique et silencieux (`FORMAT.md` §2.1).
    pub const DEFAULT: Self = Self {
        memory_kib: 131_072, // 128 MiB
        iterations: 4,
        parallelism: 4,
    };

    /// Coût mémoire maximal accepté à la lecture, en KiB.
    ///
    /// Ces bornes de sûreté (`FORMAT.md` §4.2, étape 4) sont indispensables :
    /// sans elles, un en-tête hostile annonçant 64 GiB provoquerait un déni de
    /// service avant même la vérification du tag.
    pub const MAX_MEMORY_KIB: u32 = 4 * 1024 * 1024; // 4 GiB
    /// Nombre d'itérations maximal accepté à la lecture.
    pub const MAX_ITERATIONS: u32 = 32;
    /// Degré de parallélisme maximal accepté à la lecture.
    pub const MAX_PARALLELISM: u8 = 16;

    /// Vérifie que les paramètres tiennent dans les bornes de sûreté.
    pub fn validate(&self) -> Result<()> {
        if self.parallelism == 0 {
            return Err(Error::UnsupportedKdfParameters {
                reason: "le parallélisme doit être au moins 1",
            });
        }
        if self.parallelism > Self::MAX_PARALLELISM {
            return Err(Error::UnsupportedKdfParameters {
                reason: "parallélisme au-delà de la borne de sûreté",
            });
        }
        if self.iterations == 0 {
            return Err(Error::UnsupportedKdfParameters {
                reason: "le nombre d'itérations doit être au moins 1",
            });
        }
        if self.iterations > Self::MAX_ITERATIONS {
            return Err(Error::UnsupportedKdfParameters {
                reason: "nombre d'itérations au-delà de la borne de sûreté",
            });
        }
        // Contrainte propre à Argon2 : m >= 8 * p.
        if self.memory_kib < 8 * u32::from(self.parallelism) {
            return Err(Error::UnsupportedKdfParameters {
                reason: "coût mémoire insuffisant pour ce parallélisme",
            });
        }
        if self.memory_kib > Self::MAX_MEMORY_KIB {
            return Err(Error::UnsupportedKdfParameters {
                reason: "coût mémoire au-delà de la borne de sûreté",
            });
        }
        // Une compilation 32 bits ne peut pas adresser l'allocation demandée.
        // Signalé explicitement pour ne jamais être confondu avec un mot de
        // passe incorrect.
        if usize::BITS < 64 && self.memory_kib as u64 * 1024 > usize::MAX as u64 {
            return Err(Error::UnsupportedKdfParameters {
                reason: "coût mémoire inadressable sur cette plateforme",
            });
        }
        Ok(())
    }
}

/// En-tête décodé d'un fichier v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Paramètres de dérivation.
    pub kdf: KdfParams,
    /// Sel Argon2id, unique par fichier.
    pub salt: [u8; SALT_LEN],
    /// Nonce XChaCha20, unique par fichier.
    pub nonce: [u8; NONCE_LEN],
}

impl Header {
    /// Sérialise l'en-tête en 64 octets.
    pub fn encode(&self) -> [u8; HEADER_LEN] {
        let mut out = [0u8; HEADER_LEN];
        out[0..8].copy_from_slice(&MAGIC);
        out[8] = VERSION;
        out[9] = KDF_ARGON2ID;
        out[10] = AEAD_XCHACHA20POLY1305;
        // out[11] réservé, laissé à zéro
        out[12..16].copy_from_slice(&self.kdf.memory_kib.to_be_bytes());
        out[16..20].copy_from_slice(&self.kdf.iterations.to_be_bytes());
        out[20] = self.kdf.parallelism;
        // out[21..24] réservés, laissés à zéro
        out[24..40].copy_from_slice(&self.salt);
        out[40..64].copy_from_slice(&self.nonce);
        out
    }

    /// Décode et valide l'en-tête.
    ///
    /// Effectue toutes les vérifications qui précèdent la dérivation de clé
    /// (`FORMAT.md` §4.2, étapes 2 à 4).
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(Error::InvalidLength {
                actual: bytes.len(),
            });
        }
        if bytes[0..8] != MAGIC {
            return Err(Error::NotCoocryptFile);
        }
        if bytes[8] != VERSION {
            return Err(Error::UnsupportedVersion(bytes[8]));
        }
        if bytes[9] != KDF_ARGON2ID || bytes[10] != AEAD_XCHACHA20POLY1305 {
            return Err(Error::UnsupportedAlgorithm {
                kdf: bytes[9],
                aead: bytes[10],
            });
        }
        // Les octets réservés doivent être nuls : cela préserve leur usage pour
        // une future version, qui pourra leur donner un sens sans ambiguïté.
        if bytes[11] != 0 || bytes[21..24] != [0, 0, 0] {
            return Err(Error::ReservedBytesNotZero);
        }

        let kdf = KdfParams {
            memory_kib: u32::from_be_bytes(bytes[12..16].try_into().unwrap()),
            iterations: u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
            parallelism: bytes[20],
        };
        kdf.validate()?;

        Ok(Self {
            kdf,
            salt: bytes[24..40].try_into().unwrap(),
            nonce: bytes[40..64].try_into().unwrap(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Header {
        Header {
            kdf: KdfParams::DEFAULT,
            salt: [7u8; SALT_LEN],
            nonce: [9u8; NONCE_LEN],
        }
    }

    #[test]
    fn aller_retour() {
        let h = sample();
        assert_eq!(Header::decode(&h.encode()).unwrap(), h);
    }

    #[test]
    fn en_tete_de_64_octets_exactement() {
        assert_eq!(sample().encode().len(), HEADER_LEN);
    }

    #[test]
    fn rejette_un_octet_reserve_non_nul() {
        for offset in [11usize, 21, 22, 23] {
            let mut raw = sample().encode();
            raw[offset] = 1;
            assert_eq!(
                Header::decode(&raw).unwrap_err(),
                Error::ReservedBytesNotZero,
                "offset {offset}"
            );
        }
    }

    #[test]
    fn rejette_les_parametres_hors_bornes() {
        let mut raw = sample().encode();
        raw[12..16].copy_from_slice(&(64u32 * 1024 * 1024).to_be_bytes()); // 64 GiB
        assert!(matches!(
            Header::decode(&raw).unwrap_err(),
            Error::UnsupportedKdfParameters { .. }
        ));
    }

    #[test]
    fn rejette_un_magic_inconnu() {
        let mut raw = sample().encode();
        raw[0] = b'X';
        assert_eq!(Header::decode(&raw).unwrap_err(), Error::NotCoocryptFile);
    }
}
