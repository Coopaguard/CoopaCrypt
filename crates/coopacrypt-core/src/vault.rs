//! Chiffrement et déchiffrement du format v2 (`FORMAT.md` §4).

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::header::{
    Header, KdfParams, BLOCK_LEN, HEADER_LEN, KEY_LEN, LEN_PREFIX, NONCE_LEN, SALT_LEN, TAG_LEN,
};
use crate::password;

/// Taille minimale d'un fichier v2 : en-tête + un bloc + tag.
pub const MIN_FILE_LEN: usize = HEADER_LEN + BLOCK_LEN + TAG_LEN; // 4176

/// Dérive la clé de 256 bits à partir du mot de passe préparé et du sel.
///
/// La version d'Argon2 est **forcée à 0x13**. Ne jamais s'en remettre au défaut
/// d'une bibliothèque : la version 0x10 produit une sortie différente, et un
/// fichier dérivé avec elle serait illisible ailleurs (`FORMAT.md` §2.1).
fn derive_key(
    password_bytes: &[u8],
    salt: &[u8; SALT_LEN],
    kdf: &KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>> {
    kdf.validate()?;

    let params = Params::new(
        kdf.memory_kib,
        kdf.iterations,
        u32::from(kdf.parallelism),
        Some(KEY_LEN),
    )
    .map_err(|_| Error::UnsupportedKdfParameters {
        reason: "combinaison de paramètres refusée par Argon2",
    })?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(password_bytes, salt, key.as_mut())
        .map_err(|_| Error::KdfFailure)?;

    Ok(key)
}

/// Construit le clair interne : `u32(len) || contenu || remplissage à zéro`,
/// complété au multiple de 4096 octets supérieur (`FORMAT.md` §3.4).
fn build_plaintext(content: &str) -> Result<Zeroizing<Vec<u8>>> {
    let content = content.as_bytes();
    let len = u32::try_from(content.len()).map_err(|_| Error::ContentTooLarge)?;

    let unpadded = LEN_PREFIX + content.len();
    // Division plafond, puis au moins un bloc.
    let blocks = unpadded.div_ceil(BLOCK_LEN).max(1);
    let padded = blocks
        .checked_mul(BLOCK_LEN)
        .ok_or(Error::ContentTooLarge)?;

    let mut buf = Zeroizing::new(vec![0u8; padded]);
    buf[0..LEN_PREFIX].copy_from_slice(&len.to_be_bytes());
    buf[LEN_PREFIX..unpadded].copy_from_slice(content);
    // Le reste demeure à zéro.

    Ok(buf)
}

/// Extrait le contenu du clair interne.
///
/// Valide le champ de longueur **avant** tout découpage : un champ incohérent
/// doit provoquer un rejet, jamais une lecture au-delà du tampon
/// (`FORMAT.md` §3.4, §4.2 étape 8).
fn parse_plaintext(plaintext: &[u8]) -> Result<Zeroizing<String>> {
    if plaintext.len() < LEN_PREFIX {
        return Err(Error::MalformedPlaintext);
    }

    let len = u32::from_be_bytes(plaintext[0..LEN_PREFIX].try_into().unwrap()) as usize;
    let available = plaintext.len() - LEN_PREFIX;
    if len > available {
        return Err(Error::MalformedPlaintext);
    }

    // Le remplissage n'est pas vérifié : le tag couvre déjà son intégrité, et le
    // laisser libre préserve une marge pour de futures extensions.
    let content = &plaintext[LEN_PREFIX..LEN_PREFIX + len];

    let text = core::str::from_utf8(content).map_err(|_| Error::InvalidUtf8)?;
    Ok(Zeroizing::new(text.to_owned()))
}

/// Chiffre un document avec les paramètres par défaut de cette version.
///
/// Le contenu est stocké **octet pour octet** : aucune normalisation Unicode,
/// aucune conversion de fins de ligne (`FORMAT.md` §2.1.3). Seul le mot de passe
/// est normalisé.
pub fn encrypt(content: &str, pass: &str) -> Result<Vec<u8>> {
    encrypt_with_params(content, pass, KdfParams::DEFAULT)
}

/// Chiffre un document avec des paramètres Argon2id explicites.
///
/// Réservé aux tests et aux vecteurs de référence : le chemin normal est
/// [`encrypt`], qui applique le durcissement automatique.
pub fn encrypt_with_params(content: &str, pass: &str, kdf: KdfParams) -> Result<Vec<u8>> {
    let password_bytes = password::prepare(pass)?;
    kdf.validate()?;

    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut salt)?;
    fill_random(&mut nonce)?;

    encrypt_deterministic(content, &password_bytes, kdf, salt, nonce)
}

/// Chiffre avec un sel et un nonce imposés.
///
/// **Usage strictement réservé aux vecteurs de test.** Réutiliser un couple
/// (clé, nonce) sur deux contenus différents ruine la confidentialité.
#[doc(hidden)]
pub fn encrypt_deterministic(
    content: &str,
    password_bytes: &[u8],
    kdf: KdfParams,
    salt: [u8; SALT_LEN],
    nonce: [u8; NONCE_LEN],
) -> Result<Vec<u8>> {
    let key = derive_key(password_bytes, &salt, &kdf)?;
    let header = Header { kdf, salt, nonce };
    let header_bytes = header.encode();

    let plaintext = build_plaintext(content)?;

    let cipher = XChaCha20Poly1305::new((&*key).into());
    // `encrypt` renvoie le chiffré suivi du tag de 16 octets, ce qui correspond
    // exactement à la disposition du fichier.
    let sealed = cipher
        .encrypt(
            <&XNonce>::from(&nonce),
            Payload {
                msg: &plaintext,
                aad: &header_bytes,
            },
        )
        .map_err(|_| Error::DecryptionFailed)?;

    let mut out = Vec::with_capacity(HEADER_LEN + sealed.len());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// Déchiffre un fichier au format v2.
///
/// La chaîne rendue s'efface de la mémoire lorsqu'elle sort de portée.
///
/// # Erreurs
///
/// [`Error::DecryptionFailed`] couvre indistinctement le mot de passe incorrect
/// et le fichier altéré : le chiffrement authentifié ne permet pas — et ne doit
/// pas permettre — de les séparer.
pub fn decrypt(file: &[u8], pass: &str) -> Result<Zeroizing<String>> {
    // Taille et congruence avant toute autre chose : rejet immédiat, sans coût.
    if file.len() < MIN_FILE_LEN || (file.len() - HEADER_LEN - TAG_LEN) % BLOCK_LEN != 0 {
        return Err(Error::InvalidLength { actual: file.len() });
    }

    let header = Header::decode(&file[0..HEADER_LEN])?;
    let password_bytes = password::prepare(pass)?;

    // Les paramètres viennent du fichier, jamais des défauts courants : c'est ce
    // qui permet de relire un fichier ancien.
    let key = derive_key(&password_bytes, &header.salt, &header.kdf)?;

    let cipher = XChaCha20Poly1305::new((&*key).into());
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                <&XNonce>::from(&header.nonce),
                Payload {
                    msg: &file[HEADER_LEN..],
                    aad: &file[0..HEADER_LEN],
                },
            )
            .map_err(|_| Error::DecryptionFailed)?,
    );

    parse_plaintext(&plaintext)
}

/// Description d'un fichier, lisible **sans** le mot de passe.
///
/// L'en-tête est en clair par conception : ces informations ne sont pas
/// secrètes. Elles ne disent rien du contenu, dont la longueur est masquée par
/// le remplissage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// Version du format.
    pub version: u8,
    /// Paramètres de dérivation utilisés à la dernière écriture.
    pub kdf: KdfParams,
    /// Taille totale du fichier, en octets.
    pub file_len: usize,
    /// Nombre de blocs de remplissage.
    pub blocks: usize,
}

/// Lit l'en-tête d'un fichier sans dériver de clé ni déchiffrer.
///
/// Aucun mot de passe n'est requis, et aucune information sur le contenu n'est
/// révélée au-delà de ce que la taille du fichier montre déjà.
pub fn inspect(file: &[u8]) -> Result<FileInfo> {
    if file.len() < MIN_FILE_LEN || (file.len() - HEADER_LEN - TAG_LEN) % BLOCK_LEN != 0 {
        return Err(Error::InvalidLength { actual: file.len() });
    }
    let header = Header::decode(&file[0..HEADER_LEN])?;
    Ok(FileInfo {
        version: crate::header::VERSION,
        kdf: header.kdf,
        file_len: file.len(),
        blocks: (file.len() - HEADER_LEN - TAG_LEN) / BLOCK_LEN,
    })
}

/// Indique si le fichier porte la signature du format v2.
///
/// Permet de rejeter tôt, et avec un message clair, un fichier qui n'est pas un
/// coffre — sans faire payer une dérivation Argon2id à l'utilisateur.
pub fn is_v2(file: &[u8]) -> bool {
    file.len() >= crate::header::MAGIC.len() && file[0..8] == crate::header::MAGIC
}

/// Remplit un tampon depuis la source d'entropie du système.
///
/// Un échec est remonté plutôt qu'ignoré : produire un sel ou un nonce
/// prévisible serait bien pire qu'une erreur visible.
fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::fill(buf).map_err(|_| Error::RandomSourceUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paramètres volontairement faibles : les tests ne mesurent pas la
    /// résistance du KDF, et 256 MiB par appel rendrait la suite inutilisable.
    const FAST: KdfParams = KdfParams {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };

    fn roundtrip(content: &str) {
        let file = encrypt_with_params(content, "mot de passe", FAST).unwrap();
        let out = decrypt(&file, "mot de passe").unwrap();
        assert_eq!(*out, content);
    }

    #[test]
    fn contenu_vide() {
        roundtrip("");
    }

    #[test]
    fn contenu_court() {
        roundtrip("# Chapitre\n\nDu texte.\n");
    }

    #[test]
    fn contenu_utf8_multioctets() {
        roundtrip("Accents éàü, emoji 🔐, idéogrammes 漢字");
    }

    #[test]
    fn le_contenu_n_est_pas_normalise() {
        // "é" décomposé doit ressortir décomposé, à l'octet près.
        let decompose = "cafe\u{0301}";
        let file = encrypt_with_params(decompose, "pass", FAST).unwrap();
        let out = decrypt(&file, "pass").unwrap();
        assert_eq!(out.as_bytes(), decompose.as_bytes());
        assert_ne!(out.as_bytes(), "caf\u{00E9}".as_bytes());
    }

    #[test]
    fn les_fins_de_ligne_sont_preservees() {
        let crlf = "ligne un\r\nligne deux\r\n";
        let file = encrypt_with_params(crlf, "pass", FAST).unwrap();
        assert_eq!(decrypt(&file, "pass").unwrap().as_bytes(), crlf.as_bytes());
    }

    #[test]
    fn frontieres_de_remplissage() {
        // 4092 = 4096 - 4 : dernier contenu tenant dans un bloc.
        for len in [0usize, 1, 4091, 4092, 4093, 8188, 20_000] {
            let content = "a".repeat(len);
            let file = encrypt_with_params(&content, "pass", FAST).unwrap();

            let expected_blocks = (4 + len).div_ceil(BLOCK_LEN).max(1);
            assert_eq!(
                file.len(),
                HEADER_LEN + expected_blocks * BLOCK_LEN + TAG_LEN,
                "longueur {len}"
            );
            assert_eq!(*decrypt(&file, "pass").unwrap(), content, "longueur {len}");
        }
    }

    #[test]
    fn la_taille_ne_revele_pas_la_longueur_exacte() {
        let a = encrypt_with_params("court", "pass", FAST).unwrap();
        let b = encrypt_with_params(&"x".repeat(3000), "pass", FAST).unwrap();
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn mauvais_mot_de_passe() {
        let file = encrypt_with_params("secret", "bon", FAST).unwrap();
        assert_eq!(
            decrypt(&file, "mauvais").unwrap_err(),
            Error::DecryptionFailed
        );
    }

    #[test]
    fn deux_chiffrements_du_meme_contenu_different() {
        // Sel et nonce aléatoires : aucune corrélation observable.
        let a = encrypt_with_params("identique", "pass", FAST).unwrap();
        let b = encrypt_with_params("identique", "pass", FAST).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn tag_altere() {
        let mut file = encrypt_with_params("secret", "pass", FAST).unwrap();
        let last = file.len() - 1;
        file[last] ^= 0x01;
        assert_eq!(decrypt(&file, "pass").unwrap_err(), Error::DecryptionFailed);
    }

    #[test]
    fn chiffre_altere() {
        let mut file = encrypt_with_params("secret", "pass", FAST).unwrap();
        file[HEADER_LEN] ^= 0x01;
        assert_eq!(decrypt(&file, "pass").unwrap_err(), Error::DecryptionFailed);
    }

    #[test]
    fn en_tete_altere_detecte_par_l_aad() {
        // Le sel est dans l'AAD : le modifier doit invalider le tag, et non
        // produire silencieusement un autre contenu.
        let mut file = encrypt_with_params("secret", "pass", FAST).unwrap();
        file[24] ^= 0x01;
        assert_eq!(decrypt(&file, "pass").unwrap_err(), Error::DecryptionFailed);
    }

    #[test]
    fn abaisser_les_parametres_dans_l_en_tete_echoue() {
        // Un attaquant qui affaiblirait le KDF pour accélérer une attaque doit
        // se heurter au tag. Le fichier est écrit avec t=3 pour que l'abaissement
        // à t=1 soit une modification réelle.
        let strong = KdfParams {
            iterations: 3,
            ..FAST
        };
        let mut file = encrypt_with_params("secret", "pass", strong).unwrap();
        assert_eq!(file[16..20], 3u32.to_be_bytes());

        file[16..20].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(decrypt(&file, "pass").unwrap_err(), Error::DecryptionFailed);
    }

    #[test]
    fn mot_de_passe_vide_refuse() {
        assert_eq!(
            encrypt_with_params("x", "", FAST).unwrap_err(),
            Error::EmptyPassword
        );
        let file = encrypt_with_params("x", "pass", FAST).unwrap();
        assert_eq!(decrypt(&file, "").unwrap_err(), Error::EmptyPassword);
    }

    #[test]
    fn tailles_de_fichier_invalides() {
        let file = encrypt_with_params("secret", "pass", FAST).unwrap();

        assert!(matches!(
            decrypt(&file[..file.len() - 1], "pass").unwrap_err(),
            Error::InvalidLength { .. }
        ));
        assert!(matches!(
            decrypt(&[], "pass").unwrap_err(),
            Error::InvalidLength { .. }
        ));
    }

    #[test]
    fn champ_de_longueur_incoherent() {
        // Forgé par quelqu'un qui connaît le mot de passe : le tag est valide,
        // seule la validation explicite du champ protège.
        let password_bytes = password::prepare("pass").unwrap();
        let mut plaintext = vec![0u8; BLOCK_LEN];
        plaintext[0..4].copy_from_slice(&u32::MAX.to_be_bytes());

        let salt = [1u8; SALT_LEN];
        let nonce = [2u8; NONCE_LEN];
        let key = derive_key(&password_bytes, &salt, &FAST).unwrap();
        let header = Header {
            kdf: FAST,
            salt,
            nonce,
        };
        let header_bytes = header.encode();
        let sealed = XChaCha20Poly1305::new((&*key).into())
            .encrypt(
                <&XNonce>::from(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &header_bytes,
                },
            )
            .unwrap();

        let mut file = header_bytes.to_vec();
        file.extend_from_slice(&sealed);

        assert_eq!(
            decrypt(&file, "pass").unwrap_err(),
            Error::MalformedPlaintext
        );
    }

    #[test]
    fn nfd_et_nfc_ouvrent_le_meme_fichier() {
        let file = encrypt_with_params("secret", "caf\u{00E9}", FAST).unwrap();
        assert_eq!(*decrypt(&file, "cafe\u{0301}").unwrap(), "secret");
    }

    #[test]
    fn inspection_sans_mot_de_passe() {
        let file = encrypt_with_params(&"x".repeat(5000), "pass", FAST).unwrap();
        let info = inspect(&file).unwrap();
        assert_eq!(info.version, 2);
        assert_eq!(info.kdf, FAST);
        assert_eq!(info.blocks, 2);
        assert_eq!(info.file_len, file.len());
    }

    #[test]
    fn inspection_refuse_un_fichier_invalide() {
        assert!(inspect(b"pas un coffre").is_err());
    }

    #[test]
    fn detection_de_version() {
        let file = encrypt_with_params("x", "pass", FAST).unwrap();
        assert!(is_v2(&file));
        assert!(!is_v2(b"\x00\x01\x02\x03\x04\x05\x06\x07"));
    }
}
