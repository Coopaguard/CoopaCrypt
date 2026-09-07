//! Préparation du mot de passe avant dérivation (`FORMAT.md` §2.1.3).
//!
//! Deux règles, dans cet ordre, correspondant au profil `OpaqueString` de la
//! RFC 8265 (PRECIS) :
//!
//! 1. **Mise en correspondance des espaces** : tout caractère de la catégorie
//!    Unicode `Zs` est remplacé par l'espace ASCII `U+0020`.
//! 2. **Normalisation NFC**.
//!
//! La casse, les accents, les symboles et les emoji sont intégralement préservés.
//! `NFKC` n'est **pas** utilisé : il fusionnerait des caractères que l'utilisateur
//! tient pour distincts (`ﬁ` → `fi`, `²` → `2`) et détruirait de l'entropie.
//!
//! Ces règles sont figées : elles conditionnent la reproductibilité des vecteurs
//! de test et donc la lisibilité inter-plateformes des fichiers.

use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Catégorie Unicode `Zs` (Separator, space) dans son intégralité.
///
/// Liste stable et close ; l'inscrire en dur évite une dépendance aux tables
/// Unicode et garantit que le comportement ne dérive pas avec les versions
/// d'Unicode — ce qui casserait les fichiers existants.
const UNICODE_SPACES: [char; 17] = [
    '\u{0020}', // SPACE
    '\u{00A0}', // NO-BREAK SPACE
    '\u{1680}', // OGHAM SPACE MARK
    '\u{2000}', // EN QUAD
    '\u{2001}', // EM QUAD
    '\u{2002}', // EN SPACE
    '\u{2003}', // EM SPACE
    '\u{2004}', // THREE-PER-EM SPACE
    '\u{2005}', // FOUR-PER-EM SPACE
    '\u{2006}', // SIX-PER-EM SPACE
    '\u{2007}', // FIGURE SPACE
    '\u{2008}', // PUNCTUATION SPACE
    '\u{2009}', // THIN SPACE
    '\u{200A}', // HAIR SPACE
    '\u{202F}', // NARROW NO-BREAK SPACE
    '\u{205F}', // MEDIUM MATHEMATICAL SPACE
    '\u{3000}', // IDEOGRAPHIC SPACE
];

/// Normalise un mot de passe et le rend sous forme d'octets UTF-8.
///
/// La valeur retournée s'efface de la mémoire lorsqu'elle sort de portée.
///
/// # Erreurs
///
/// [`Error::EmptyPassword`] si le mot de passe est vide.
pub(crate) fn prepare(password: &str) -> Result<Zeroizing<Vec<u8>>> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }

    let mapped: Zeroizing<String> = Zeroizing::new(
        password
            .chars()
            .map(|c| if UNICODE_SPACES.contains(&c) { ' ' } else { c })
            .collect(),
    );

    let normalized: Zeroizing<String> = Zeroizing::new(mapped.nfc().collect());

    Ok(Zeroizing::new(normalized.as_bytes().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejette_le_mot_de_passe_vide() {
        assert_eq!(prepare("").unwrap_err(), Error::EmptyPassword);
    }

    #[test]
    fn nfd_et_nfc_convergent() {
        // "é" précomposé U+00E9 contre "e" + U+0301 combinant.
        let precompose = prepare("caf\u{00E9}").unwrap();
        let decompose = prepare("cafe\u{0301}").unwrap();
        assert_eq!(*precompose, *decompose);
    }

    #[test]
    fn nfkc_n_est_pas_applique() {
        // Si NFKC était appliqué, ces trois formes fusionneraient.
        let ligature = prepare("\u{FB01}n").unwrap(); // ﬁn
        let ascii = prepare("fin").unwrap();
        assert_ne!(*ligature, *ascii);

        let exposant = prepare("x\u{00B2}").unwrap(); // x²
        let chiffre = prepare("x2").unwrap();
        assert_ne!(*exposant, *chiffre);
    }

    #[test]
    fn les_espaces_unicode_deviennent_ascii() {
        let insecable = prepare("mot\u{00A0}de passe").unwrap();
        let ascii = prepare("mot de passe").unwrap();
        assert_eq!(*insecable, *ascii);
    }

    #[test]
    fn la_casse_et_les_emoji_sont_preserves() {
        assert_ne!(*prepare("Secret").unwrap(), *prepare("secret").unwrap());
        assert_eq!(*prepare("clé🔐").unwrap(), "clé🔐".as_bytes().to_vec());
    }
}
