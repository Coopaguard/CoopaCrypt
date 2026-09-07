//! Conformité aux vecteurs de référence.
//!
//! `vectors.json` est le contrat du format : toute implémentation, dans
//! n'importe quel langage, doit le reproduire à l'octet près.
//!
//! **Si un de ces tests échoue, le format a changé.** Le réflexe correct est de
//! trouver ce qui a bougé dans le code, jamais de régénérer le fichier.

use coopacrypt_core::KdfParams;
use serde_json::Value;

fn vectors() -> Vec<Value> {
    let raw = include_str!("vectors.json");
    let doc: Value = serde_json::from_str(raw).expect("vectors.json illisible");
    assert_eq!(doc["format_version"], 2);
    doc["vectors"].as_array().expect("tableau attendu").clone()
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hexadécimal invalide"))
        .collect()
}

fn field<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("champ « {key} » manquant"))
}

/// Chaque vecteur doit être reproduit octet pour octet à partir de ses entrées.
#[test]
fn les_fichiers_sont_reproduits_a_l_octet_pres() {
    for v in vectors() {
        let name = field(&v, "name");

        let kdf = KdfParams {
            memory_kib: v["kdf"]["memory_kib"].as_u64().unwrap() as u32,
            iterations: v["kdf"]["iterations"].as_u64().unwrap() as u32,
            parallelism: v["kdf"]["parallelism"].as_u64().unwrap() as u8,
        };
        assert_eq!(v["kdf"]["version"], "0x13", "[{name}] version d'Argon2");

        let salt: [u8; 16] = hex_to_bytes(field(&v, "salt_hex")).try_into().unwrap();
        let nonce: [u8; 24] = hex_to_bytes(field(&v, "nonce_hex")).try_into().unwrap();
        let content = String::from_utf8(hex_to_bytes(field(&v, "content_hex"))).unwrap();

        let prepared = coopacrypt_core::prepare_password(field(&v, "password")).unwrap();
        assert_eq!(
            hex(&prepared),
            field(&v, "password_prepared_hex"),
            "[{name}] préparation du mot de passe"
        );

        let produced =
            coopacrypt_core::encrypt_deterministic(&content, &prepared, kdf, salt, nonce).unwrap();

        assert_eq!(
            hex(&produced),
            field(&v, "file_hex"),
            "[{name}] le fichier produit diffère du vecteur"
        );
        assert_eq!(
            produced.len(),
            v["file_len"].as_u64().unwrap() as usize,
            "[{name}] taille de fichier"
        );
    }
}

/// Chaque vecteur doit se relire avec son mot de passe d'origine.
#[test]
fn les_fichiers_se_relisent() {
    for v in vectors() {
        let name = field(&v, "name");
        let file = hex_to_bytes(field(&v, "file_hex"));
        let expected = hex_to_bytes(field(&v, "content_hex"));

        let out = coopacrypt_core::decrypt(&file, field(&v, "password"))
            .unwrap_or_else(|e| panic!("[{name}] déchiffrement échoué : {e}"));

        // Comparaison sur les octets, seule façon de détecter une normalisation
        // parasite du contenu.
        assert_eq!(out.as_bytes(), expected.as_slice(), "[{name}] contenu");
    }
}

/// NFC et NFD du même mot de passe doivent produire les mêmes octets préparés.
#[test]
fn nfc_et_nfd_convergent() {
    let all = vectors();
    let nfc = find(&all, "mdp-nfc");
    let nfd = find(&all, "mdp-nfd");

    assert_eq!(
        field(&nfc, "password_prepared_hex"),
        field(&nfd, "password_prepared_hex"),
        "la normalisation NFC n'est pas appliquée au mot de passe"
    );

    // Vérification de bout en bout : le fichier écrit en NFC s'ouvre en NFD.
    let file = hex_to_bytes(field(&nfc, "file_hex"));
    assert!(coopacrypt_core::decrypt(&file, field(&nfd, "password")).is_ok());
}

/// NFKC ne doit PAS être appliqué : une ligature reste distincte de son
/// équivalent ASCII, sans quoi de l'entropie serait détruite.
#[test]
fn nfkc_n_est_pas_applique() {
    let all = vectors();
    let ligature = find(&all, "mdp-ligature");
    let temoin = find(&all, "mdp-ligature-temoin");

    assert_ne!(
        field(&ligature, "password_prepared_hex"),
        field(&temoin, "password_prepared_hex"),
        "NFKC semble appliqué : « ﬁn » et « fin » ont fusionné"
    );

    let file = hex_to_bytes(field(&ligature, "file_hex"));
    assert!(coopacrypt_core::decrypt(&file, field(&temoin, "password")).is_err());
}

/// Le contenu ne doit subir aucune normalisation ni conversion de fins de ligne.
#[test]
fn le_contenu_traverse_intact() {
    let all = vectors();

    let nfd = find(&all, "contenu-nfd-preserve");
    let file = hex_to_bytes(field(&nfd, "file_hex"));
    let out = coopacrypt_core::decrypt(&file, field(&nfd, "password")).unwrap();
    assert_eq!(out.as_bytes(), "cafe\u{0301}".as_bytes());
    assert_ne!(out.as_bytes(), "caf\u{00E9}".as_bytes());

    let crlf = find(&all, "contenu-crlf-preserve");
    let file = hex_to_bytes(field(&crlf, "file_hex"));
    let out = coopacrypt_core::decrypt(&file, field(&crlf, "password")).unwrap();
    assert!(out.contains("\r\n"), "les CRLF ont été convertis");
}

/// Le remplissage masque la longueur : deux contenus de tailles très
/// différentes tenant dans un bloc donnent des fichiers de même taille.
#[test]
fn le_remplissage_masque_la_longueur() {
    let all = vectors();
    let vide = find(&all, "contenu-vide");
    let plein = find(&all, "frontiere-4092");

    assert_eq!(vide["file_len"], 4176);
    assert_eq!(plein["file_len"], 4176);
    assert_eq!(find(&all, "frontiere-4093")["file_len"], 8272);
}

/// Un vecteur au moins doit figer les paramètres réellement utilisés en
/// production, et non seulement les paramètres réduits des tests.
#[test]
fn les_parametres_par_defaut_sont_figes() {
    let all = vectors();
    let v = find(&all, "parametres-par-defaut");
    assert_eq!(v["kdf"]["memory_kib"], 131_072);
    assert_eq!(v["kdf"]["iterations"], 4);
    assert_eq!(v["kdf"]["parallelism"], 4);
    assert_eq!(
        KdfParams::DEFAULT,
        KdfParams {
            memory_kib: 131_072,
            iterations: 4,
            parallelism: 4,
        },
        "les paramètres par défaut du code ont changé sans mise à jour du vecteur"
    );
}

fn find(all: &[Value], name: &str) -> Value {
    all.iter()
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("vecteur « {name} » absent"))
        .clone()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
