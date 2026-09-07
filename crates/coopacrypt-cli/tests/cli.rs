//! Tests d'intégration de la CLI, exécutée comme un vrai processus.
//!
//! Ces tests sont marqués `#[ignore]` : chacun déclenche plusieurs dérivations
//! Argon2id aux paramètres réels (256 MiB), ce qui est rapide en `release` mais
//! très lent en `debug`. Ils ne doivent donc pas alourdir un `cargo test`
//! ordinaire.
//!
//! ```text
//! cargo test --release -- --ignored
//! ```
//!
//! Volontairement, aucun réglage permettant d'affaiblir le KDF n'est exposé : un
//! binaire de production ne doit pas embarquer de bouton « chiffrer moins bien »,
//! même réservé aux tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_coopacrypt");
const PASS: &str = "correcte cheval batterie agrafe zeste";

/// Répertoire de travail isolé, nettoyé s'il subsiste d'une exécution passée.
fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("coopacrypt-it-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("création du répertoire de test");
    dir
}

/// Lance la CLI en lui fournissant les mots de passe sur l'entrée standard.
fn run(dir: &Path, args: &[&str], stdin_lines: &[&str]) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("lancement de la CLI");

    {
        let stdin = child.stdin.as_mut().expect("entrée standard");
        for line in stdin_lines {
            writeln!(stdin, "{line}").expect("écriture sur l'entrée standard");
        }
    }

    child.wait_with_output().expect("attente de la CLI")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
#[ignore = "dérivations Argon2id réelles ; lancer avec --release -- --ignored"]
fn aller_retour_a_l_octet_pres() {
    let dir = workspace("roundtrip");
    // Contenu volontairement piégeux : accents, emoji, CRLF, ligne sans fin de ligne.
    let source = "# Comptes\r\n\r\nmot de passe : café-noir 🔐\r\nfin sans retour";
    fs::write(dir.join("notes.md"), source).unwrap();

    let out = run(
        &dir,
        &["encrypt", "notes.md", "-o", "coffre.coocrypt"],
        &[PASS, PASS],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    let vault = fs::read(dir.join("coffre.coocrypt")).unwrap();
    assert_eq!(vault.len(), 4176, "un bloc de remplissage attendu");
    assert_eq!(&vault[0..8], b"COOCRYPT");

    let out = run(&dir, &["decrypt", "coffre.coocrypt"], &[PASS]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        out.stdout,
        source.as_bytes(),
        "le contenu a été altéré en transitant par le coffre"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
#[ignore = "dérivations Argon2id réelles ; lancer avec --release -- --ignored"]
fn mauvais_mot_de_passe_et_alteration_sont_rejetes() {
    let dir = workspace("rejets");
    fs::write(dir.join("notes.md"), "secret").unwrap();
    let out = run(
        &dir,
        &["encrypt", "notes.md", "-o", "coffre.coocrypt"],
        &[PASS, PASS],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    let out = run(
        &dir,
        &["decrypt", "coffre.coocrypt"],
        &["un autre mot de passe"],
    );
    assert!(
        !out.status.success(),
        "un mauvais mot de passe a été accepté"
    );
    assert!(stderr(&out).contains("incorrect ou fichier altéré"));

    // Un seul octet du chiffré retourné doit suffire à invalider le tag.
    let mut vault = fs::read(dir.join("coffre.coocrypt")).unwrap();
    vault[3000] ^= 0x01;
    fs::write(dir.join("altere.coocrypt"), &vault).unwrap();

    let out = run(&dir, &["decrypt", "altere.coocrypt"], &[PASS]);
    assert!(!out.status.success(), "une altération est passée inaperçue");

    fs::remove_dir_all(&dir).ok();
}

#[test]
#[ignore = "dérivations Argon2id réelles ; lancer avec --release -- --ignored"]
fn changement_de_mot_de_passe() {
    let dir = workspace("chpass");
    const NOUVEAU: &str = "tortue papier lampe soleil nuage";

    fs::write(dir.join("notes.md"), "# Coffre\ncontenu").unwrap();
    run(
        &dir,
        &["encrypt", "notes.md", "-o", "coffre.coocrypt"],
        &[PASS, PASS],
    );

    let out = run(
        &dir,
        &["chpass", "coffre.coocrypt"],
        &[PASS, NOUVEAU, NOUVEAU],
    );
    assert!(out.status.success(), "{}", stderr(&out));

    let out = run(&dir, &["decrypt", "coffre.coocrypt"], &[NOUVEAU]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(out.stdout, b"# Coffre\ncontenu");

    let out = run(&dir, &["decrypt", "coffre.coocrypt"], &[PASS]);
    assert!(
        !out.status.success(),
        "l'ancien mot de passe fonctionne encore"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
#[ignore = "dérivations Argon2id réelles ; lancer avec --release -- --ignored"]
fn le_remplissage_masque_la_longueur() {
    let dir = workspace("padding");
    fs::write(dir.join("court.md"), "a").unwrap();
    fs::write(dir.join("long.md"), "b".repeat(3000)).unwrap();

    run(
        &dir,
        &["encrypt", "court.md", "-o", "a.coocrypt"],
        &[PASS, PASS],
    );
    run(
        &dir,
        &["encrypt", "long.md", "-o", "b.coocrypt"],
        &[PASS, PASS],
    );

    assert_eq!(
        fs::metadata(dir.join("a.coocrypt")).unwrap().len(),
        fs::metadata(dir.join("b.coocrypt")).unwrap().len(),
        "la taille du fichier trahit la longueur du contenu"
    );

    fs::remove_dir_all(&dir).ok();
}

/// Ces refus interviennent **avant** toute dérivation : ils sont donc rapides et
/// n'ont pas besoin d'être ignorés par défaut.
#[test]
fn refus_immediats_sans_derivation() {
    let dir = workspace("refus");
    fs::write(dir.join("quelconque.txt"), "ceci n'est pas un coffre").unwrap();

    // `info` ne demande jamais de mot de passe.
    let out = run(&dir, &["info", "quelconque.txt"], &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("n'est pas un coffre"));

    // Un coffre existant n'est jamais écrasé : il n'en existe aucune sauvegarde.
    fs::write(dir.join("deja.coocrypt"), "peu importe").unwrap();
    let out = run(
        &dir,
        &["encrypt", "quelconque.txt", "-o", "deja.coocrypt"],
        &[PASS, PASS],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("existe déjà"));

    // Une confirmation divergente est détectée avant la moindre écriture.
    let out = run(
        &dir,
        &["new", "neuf.coocrypt"],
        &["phrase de passe une", "phrase de passe deux"],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("diffèrent"));
    assert!(
        !dir.join("neuf.coocrypt").exists(),
        "un fichier a été créé malgré l'échec"
    );

    // Mot de passe vide.
    let out = run(&dir, &["new", "vide.coocrypt"], &[""]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("ne peut pas être vide"));

    fs::remove_dir_all(&dir).ok();
}
