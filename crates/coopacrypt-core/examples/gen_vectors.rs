//! Génère les vecteurs de test de référence du format v2.
//!
//! ```text
//! cargo run --release --example gen_vectors
//! ```
//!
//! Écrit `crates/coopacrypt-core/tests/vectors.json`, qui est **gelé** une fois
//! produit : toute implémentation du format doit le reproduire à l'octet près.
//! Le régénérer revient à modifier le format — à ne faire que délibérément, et
//! jamais pour faire passer un test qui échoue.

use coopacrypt_core::KdfParams;
use serde_json::{json, Map, Value};

/// Paramètres réduits pour la majorité des vecteurs.
///
/// Les vecteurs figent la **structure** du format, pas la résistance du KDF :
/// utiliser 256 MiB partout rendrait la vérification interminable sans rien
/// prouver de plus. Un vecteur dédié couvre les paramètres réels.
const SMALL: KdfParams = KdfParams {
    memory_kib: 64,
    iterations: 1,
    parallelism: 1,
};

struct Spec<'a> {
    name: &'static str,
    note: &'static str,
    password: &'static str,
    content: &'a str,
    kdf: KdfParams,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Contenus construits à l'exécution pour les cas de frontière.
    let bord_4092 = "a".repeat(4092);
    let bord_4093 = "a".repeat(4093);

    let specs: Vec<Spec<'_>> = vec![
        Spec {
            name: "contenu-vide",
            note: "Le fichier minimal : un bloc de remplissage complet.",
            password: "phrase de passe",
            content: "",
            kdf: SMALL,
        },
        Spec {
            name: "ascii-court",
            note: "Cas nominal.",
            password: "phrase de passe",
            content: "# Chapitre\n\nDu texte.\n",
            kdf: SMALL,
        },
        Spec {
            name: "utf8-multioctets",
            note: "Accents, emoji et idéogrammes dans le contenu.",
            password: "phrase de passe",
            content: "Accents éàü, emoji 🔐, idéogrammes 漢字",
            kdf: SMALL,
        },
        Spec {
            name: "contenu-nfd-preserve",
            note: "Le contenu n'est JAMAIS normalisé : « é » décomposé doit rester décomposé.",
            password: "phrase de passe",
            content: "cafe\u{0301}",
            kdf: SMALL,
        },
        Spec {
            name: "contenu-crlf-preserve",
            note: "Les fins de ligne font partie du contenu.",
            password: "phrase de passe",
            content: "ligne un\r\nligne deux\r\n",
            kdf: SMALL,
        },
        Spec {
            name: "frontiere-4092",
            note: "4092 = 4096 - 4 : dernier contenu tenant dans un seul bloc.",
            password: "phrase de passe",
            content: &bord_4092,
            kdf: SMALL,
        },
        Spec {
            name: "frontiere-4093",
            note: "Un octet de plus : bascule sur deux blocs.",
            password: "phrase de passe",
            content: &bord_4093,
            kdf: SMALL,
        },
        Spec {
            name: "mdp-nfc",
            note: "Mot de passe accentué en forme précomposée (NFC).",
            password: "caf\u{00E9} au lait",
            content: "secret",
            kdf: SMALL,
        },
        Spec {
            name: "mdp-nfd",
            note: "Même mot de passe en forme décomposée (NFD). \
                   Les octets préparés doivent être IDENTIQUES à ceux de « mdp-nfc ».",
            password: "cafe\u{0301} au lait",
            content: "secret",
            kdf: SMALL,
        },
        Spec {
            name: "mdp-espace-insecable",
            note: "L'espace insécable U+00A0 est ramené à l'espace ASCII.",
            password: "mot\u{00A0}de passe",
            content: "secret",
            kdf: SMALL,
        },
        Spec {
            name: "mdp-ligature",
            note: "NFKC n'est PAS appliqué : « ﬁn » doit rester distinct de « fin ». \
                   Les octets préparés diffèrent de ceux de « mdp-ligature-temoin ».",
            password: "\u{FB01}n du test",
            content: "secret",
            kdf: SMALL,
        },
        Spec {
            name: "mdp-ligature-temoin",
            note: "Témoin ASCII du vecteur précédent.",
            password: "fin du test",
            content: "secret",
            kdf: SMALL,
        },
        Spec {
            name: "parametres-par-defaut",
            note: "Seul vecteur utilisant les paramètres réels : 256 MiB, t=4, p=4.",
            password: "correcte cheval batterie agrafe zeste",
            content: "# Coffre\n\nContenu de référence.\n",
            kdf: KdfParams::DEFAULT,
        },
    ];

    let mut vectors = Vec::new();

    for (index, spec) in specs.iter().enumerate() {
        // Sel et nonce dérivés de l'index : reproductibles, et distincts d'un
        // vecteur à l'autre.
        let salt = [(0xA0 + index) as u8; coopacrypt_core::SALT_LEN];
        let nonce = [(0xB0 + index) as u8; coopacrypt_core::NONCE_LEN];

        let prepared = coopacrypt_core::prepare_password(spec.password)?;
        let file =
            coopacrypt_core::encrypt_deterministic(spec.content, &prepared, spec.kdf, salt, nonce)?;

        // Contrôle immédiat : un vecteur qui ne se relit pas ne doit pas être écrit.
        let back = coopacrypt_core::decrypt(&file, spec.password)?;
        assert_eq!(
            *back, spec.content,
            "vecteur « {} » non réversible",
            spec.name
        );

        let mut entry = Map::new();
        entry.insert("name".into(), json!(spec.name));
        entry.insert("note".into(), json!(spec.note));
        entry.insert("password".into(), json!(spec.password));
        entry.insert("password_prepared_hex".into(), json!(hex(&prepared)));
        entry.insert(
            "kdf".into(),
            json!({
                "algorithm": "argon2id",
                "version": "0x13",
                "memory_kib": spec.kdf.memory_kib,
                "iterations": spec.kdf.iterations,
                "parallelism": spec.kdf.parallelism,
            }),
        );
        entry.insert("salt_hex".into(), json!(hex(&salt)));
        entry.insert("nonce_hex".into(), json!(hex(&nonce)));
        // Le contenu est donné en hexadécimal : aucune ambiguïté d'échappement,
        // et les séquences Unicode restent inspectables octet par octet.
        entry.insert("content_hex".into(), json!(hex(spec.content.as_bytes())));
        entry.insert("content_len".into(), json!(spec.content.len()));
        entry.insert("file_len".into(), json!(file.len()));
        entry.insert("file_hex".into(), json!(hex(&file)));

        vectors.push(Value::Object(entry));
    }

    let document = json!({
        "_comment": "Vecteurs de référence du format .coocrypt v2. GELÉS : \
                     toute implémentation doit les reproduire à l'octet près. \
                     Générés par `cargo run --release --example gen_vectors`.",
        "format_version": 2,
        "aead": "xchacha20-poly1305",
        "kdf": "argon2id-0x13",
        "padding_block": coopacrypt_core::BLOCK_LEN,
        "vectors": vectors,
    });

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors.json");
    std::fs::write(path, serde_json::to_string_pretty(&document)? + "\n")?;
    println!("{} vecteurs écrits dans {path}", specs.len());
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
