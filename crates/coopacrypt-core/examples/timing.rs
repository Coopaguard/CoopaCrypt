//! Mesure le coût réel des paramètres Argon2id par défaut.
//!
//! `cargo run --release --example timing`

use std::time::Instant;

fn main() -> Result<(), coopacrypt_core::Error> {
    let content = "# Notes\n\nDu contenu secret.\n".repeat(40);
    let pass = "correcte cheval batterie agrafe zeste";

    let t0 = Instant::now();
    let file = coopacrypt_core::encrypt(&content, pass)?;
    let write = t0.elapsed();

    let t1 = Instant::now();
    let out = coopacrypt_core::decrypt(&file, pass)?;
    let read = t1.elapsed();

    assert_eq!(*out, content);
    println!("contenu    : {} octets", content.len());
    println!("fichier    : {} octets", file.len());
    println!("chiffrement: {write:.2?}");
    println!("lecture    : {read:.2?}");
    Ok(())
}
