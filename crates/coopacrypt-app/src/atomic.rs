//! Écriture atomique (`FORMAT.md` §4.1, étape 7).
//!
//! Le coffre peut résider dans un dossier synchronisé, lu à tout instant par un
//! autre processus. Une écriture en place laisserait une fenêtre pendant
//! laquelle le fichier est tronqué — et une coupure à cet instant détruirait le
//! coffre sans recours, puisqu'il n'en existe aucune sauvegarde.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

/// Écrit `data` dans `path` sans jamais exposer d'état intermédiaire.
pub fn write(path: &Path, data: &[u8]) -> io::Result<()> {
    // Le temporaire doit vivre dans le même répertoire que la cible : un
    // remplacement n'est atomique qu'au sein d'un même système de fichiers.
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::other("chemin de destination sans nom de fichier"))?
        .to_string_lossy()
        .into_owned();
    let temp = directory.join(format!(".{file_name}.tmp"));

    {
        let mut handle = File::create(&temp)?;
        handle.write_all(data)?;
        // Sans cette synchronisation, le remplacement peut précéder l'écriture
        // effective sur le support : une coupure de courant laisserait un
        // fichier de la bonne taille, rempli de zéros.
        handle.sync_all()?;
    }

    fs::rename(&temp, path).inspect_err(|_| {
        // Ne pas laisser traîner un temporaire contenant du chiffré.
        let _ = fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecrit_puis_remplace_sans_laisser_de_temporaire() {
        let dir = std::env::temp_dir().join("coopacrypt-app-atomic");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cible = dir.join("coffre.coocrypt");

        write(&cible, b"premiere").unwrap();
        assert_eq!(fs::read(&cible).unwrap(), b"premiere");

        write(&cible, b"seconde").unwrap();
        assert_eq!(fs::read(&cible).unwrap(), b"seconde");

        let restes: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(restes.is_empty(), "temporaire non nettoyé");

        fs::remove_dir_all(&dir).ok();
    }
}
