//! `coopacrypt` — outil en ligne de commande pour les coffres `.coocrypt`.
//!
//! Sert de banc d'essai du format et de premier livrable utilisable, notamment
//! sur Linux et Linux musl où aucune interface graphique n'est nécessaire.

mod atomic;

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(
    name = "coopacrypt",
    version,
    about = "Coffre de texte chiffré — format .coocrypt v2",
    long_about = "Chiffre un document markdown avec Argon2id et XChaCha20-Poly1305.\n\
                  Le fichier produit est autonome : aucune infrastructure, aucun trousseau\n\
                  système. Il se déplace sur clé USB ou via un cloud sans perte de sécurité."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Crée un coffre vide.
    New {
        /// Fichier `.coocrypt` à créer.
        file: PathBuf,
    },

    /// Chiffre un fichier texte vers un coffre.
    Encrypt {
        /// Fichier source en clair, ou `-` pour l'entrée standard.
        source: PathBuf,
        /// Coffre de destination.
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Déchiffre un coffre.
    Decrypt {
        /// Coffre à ouvrir.
        file: PathBuf,
        /// Fichier de destination. Par défaut, la sortie standard.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Affiche l'en-tête d'un coffre, sans mot de passe.
    Info {
        /// Coffre à inspecter.
        file: PathBuf,
    },

    /// Change le mot de passe d'un coffre.
    Chpass {
        /// Coffre à ré-encoder.
        file: PathBuf,
    },
}

fn main() {
    if let Err(error) = run() {
        // `{:#}` déplie la chaîne de contextes anyhow.
        eprintln!("erreur : {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match Cli::parse().command {
        Command::New { file } => new(&file),
        Command::Encrypt { source, output } => encrypt(&source, &output),
        Command::Decrypt { file, output } => decrypt(&file, output.as_deref()),
        Command::Info { file } => info(&file),
        Command::Chpass { file } => chpass(&file),
    }
}

fn new(path: &Path) -> Result<()> {
    refuse_overwrite(path)?;
    let password = ask_new_password()?;
    let file = coopacrypt_core::encrypt("", &password)?;
    atomic::write(path, &file)?;
    eprintln!("coffre créé : {} ({} octets)", path.display(), file.len());
    Ok(())
}

fn encrypt(source: &Path, output: &Path) -> Result<()> {
    let content = read_plaintext(source)?;
    refuse_overwrite(output)?;

    let password = ask_new_password()?;
    let file = coopacrypt_core::encrypt(&content, &password)?;
    atomic::write(output, &file)?;

    eprintln!(
        "chiffré : {} → {} ({} octets, {} bloc(s))",
        source.display(),
        output.display(),
        file.len(),
        (file.len() - 80) / coopacrypt_core::BLOCK_LEN
    );
    Ok(())
}

fn decrypt(path: &Path, output: Option<&Path>) -> Result<()> {
    let file = read_vault(path)?;
    let password = ask_password("Mot de passe : ")?;
    let content = coopacrypt_core::decrypt(&file, &password)?;

    match output {
        Some(destination) => {
            refuse_overwrite(destination)?;
            atomic::write(destination, content.as_bytes())?;
            eprintln!("déchiffré vers {}", destination.display());
        }
        None => {
            if std::io::stdout().is_terminal() {
                eprintln!("--- contenu déchiffré ---");
            }
            std::io::stdout().write_all(content.as_bytes())?;
            std::io::stdout().flush()?;
        }
    }
    Ok(())
}

fn info(path: &Path) -> Result<()> {
    let file = read_vault(path)?;
    let info = coopacrypt_core::inspect(&file)?;

    println!("fichier          : {}", path.display());
    println!("format           : v{}", info.version);
    println!("chiffrement      : XChaCha20-Poly1305");
    println!("dérivation       : Argon2id v0x13");
    println!("  coût mémoire   : {} KiB", info.kdf.memory_kib);
    println!("  itérations     : {}", info.kdf.iterations);
    println!("  parallélisme   : {}", info.kdf.parallelism);
    println!("taille           : {} octets", info.file_len);
    println!("blocs            : {}", info.blocks);

    if info.kdf != coopacrypt_core::KdfParams::DEFAULT {
        println!();
        println!("note : les paramètres diffèrent des défauts de cette version.");
        println!("       Le prochain enregistrement les relèvera automatiquement.");
    }
    Ok(())
}

fn chpass(path: &Path) -> Result<()> {
    let file = read_vault(path)?;

    let current = ask_password("Mot de passe actuel : ")?;
    let content = coopacrypt_core::decrypt(&file, &current)?;

    let next = ask_new_password()?;
    // Re-chiffrement complet (`FORMAT.md` §6) : nouveau sel, nouveau nonce, et
    // au passage les paramètres par défaut de la version courante.
    let rewritten = coopacrypt_core::encrypt(&content, &next)?;
    atomic::write(path, &rewritten)?;

    eprintln!("mot de passe changé pour {}", path.display());
    Ok(())
}

// --- entrées / sorties -----------------------------------------------------

fn read_vault(path: &Path) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path).with_context(|| format!("lecture de {}", path.display()))?;

    // Contrôle avant toute saisie : inutile de demander un mot de passe pour un
    // fichier qui n'est pas un coffre.
    if !coopacrypt_core::is_v2(&bytes) {
        bail!(
            "{} n'est pas un coffre .coocrypt (signature absente)",
            path.display()
        );
    }
    Ok(bytes)
}

fn read_plaintext(source: &Path) -> Result<Zeroizing<String>> {
    if source.as_os_str() == "-" {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        return Ok(Zeroizing::new(buffer));
    }
    let text = std::fs::read_to_string(source)
        .with_context(|| format!("lecture de {}", source.display()))?;
    Ok(Zeroizing::new(text))
}

/// Refuse d'écraser un fichier existant.
///
/// Il n'existe aucune sauvegarde d'un coffre : un écrasement accidentel est
/// définitif. Mieux vaut une erreur qu'une perte silencieuse.
fn refuse_overwrite(path: &Path) -> Result<()> {
    if path.exists() {
        bail!(
            "{} existe déjà — supprimez-le explicitement pour le remplacer",
            path.display()
        );
    }
    Ok(())
}

// --- saisie du mot de passe ------------------------------------------------

/// Demande un mot de passe.
///
/// Sur un terminal, la saisie est masquée. Si l'entrée standard est redirigée,
/// une ligne y est lue à la place : c'est ce qui rend la CLI scriptable et
/// testable en intégration continue.
///
/// Cette seconde voie est réservée à l'automatisation. Un mot de passe transmis
/// par un pipe peut apparaître dans un historique de commandes ou un journal de
/// CI — ne jamais l'utiliser pour un coffre réel.
fn ask_password(prompt: &str) -> Result<Zeroizing<String>> {
    let stdin = std::io::stdin();

    let value = if stdin.is_terminal() {
        Zeroizing::new(rpassword::prompt_password(prompt)?)
    } else {
        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            bail!("mot de passe attendu sur l'entrée standard");
        }
        // Seule la fin de ligne est retirée : un mot de passe peut légitimement
        // commencer ou finir par une espace.
        Zeroizing::new(line.trim_end_matches(['\r', '\n']).to_owned())
    };

    if value.is_empty() {
        bail!("le mot de passe ne peut pas être vide");
    }
    Ok(value)
}

/// Demande un nouveau mot de passe, avec confirmation.
///
/// Une faute de frappe non détectée rend le coffre définitivement inaccessible :
/// il n'y a ni récupération, ni indice, ni question secrète.
fn ask_new_password() -> Result<Zeroizing<String>> {
    let first = ask_password("Nouveau mot de passe : ")?;
    let second = ask_password("Confirmation         : ")?;

    if first.as_str() != second.as_str() {
        bail!("les deux saisies diffèrent");
    }

    warn_if_weak(&first);
    Ok(first)
}

/// Avertit sans bloquer.
///
/// La robustesse réelle tient à l'entropie de la phrase de passe, pas au
/// chiffrement (`FORMAT.md` §2.1.2). Mais le choix appartient à l'utilisateur :
/// on informe, on n'impose pas.
fn warn_if_weak(password: &str) {
    let words = password.split_whitespace().count();
    if password.chars().count() < 12 || words < 3 {
        eprintln!();
        eprintln!("avertissement : phrase de passe courte.");
        eprintln!("  Le chiffrement ne compense pas un mot de passe devinable.");
        eprintln!("  Cinq mots tirés au hasard offrent une marge confortable.");
        eprintln!();
    }
}
