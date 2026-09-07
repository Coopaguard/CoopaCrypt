//! Écriture atomique (`FORMAT.md` §4.1, étape 7).
//!
//! Le coffre peut résider dans un dossier synchronisé, lu à tout instant par un
//! autre processus. Une écriture en place laisserait une fenêtre pendant
//! laquelle le fichier est tronqué — et un plantage à cet instant détruirait le
//! coffre sans recours, puisqu'il n'existe aucune sauvegarde.
//!
//! La séquence retenue — fichier temporaire, `sync_all`, remplacement — garantit
//! qu'un lecteur voit soit l'ancien contenu, soit le nouveau, jamais un état
//! intermédiaire.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Écrit `data` dans `path` sans jamais exposer d'état intermédiaire.
pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    // Le temporaire doit vivre dans le même répertoire que la cible : un
    // remplacement n'est atomique qu'au sein d'un même système de fichiers.
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .context("chemin de destination sans nom de fichier")?
        .to_string_lossy()
        .into_owned();
    let temp = directory.join(format!(".{file_name}.tmp"));

    {
        let mut handle = File::create(&temp)
            .with_context(|| format!("création du fichier temporaire {}", temp.display()))?;
        handle.write_all(data)?;
        // Sans cette synchronisation, le remplacement peut précéder l'écriture
        // effective des données sur le support : une coupure de courant laisserait
        // un fichier de la bonne taille, rempli de zéros.
        handle.sync_all()?;
    }

    // `rename` écrase la cible atomiquement sous Unix comme sous Windows
    // (Rust utilise MoveFileEx avec MOVEFILE_REPLACE_EXISTING).
    fs::rename(&temp, path).map_err(|e| {
        // Ne pas laisser traîner un temporaire contenant du chiffré si le
        // remplacement a échoué.
        let _ = fs::remove_file(&temp);
        e
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecrit_et_remplace() {
        let dir = std::env::temp_dir().join("coopacrypt-test-atomic");
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("coffre.coocrypt");

        write(&target, b"premiere version").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"premiere version");

        write(&target, b"seconde").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"seconde");

        // Aucun temporaire ne doit subsister.
        let restes: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(restes.is_empty(), "temporaire non nettoyé");

        fs::remove_dir_all(&dir).ok();
    }
}
