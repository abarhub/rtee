use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

use std::io::BufWriter;

struct MultiWriter<W: Write> {
    writers: Vec<W>,
}

impl<W: Write> MultiWriter<W> {
    fn new(writers: Vec<W>) -> Self {
        Self { writers }
    }
}

// L'implémentation de Drop s'occupe du nettoyage
impl<W: Write> Drop for MultiWriter<W> {
    fn drop(&mut self) {
        // On tente de flusher chaque writer avant la destruction
        for w in &mut self.writers {
            let _ = w.flush();
        }
    }
}

// On implémente aussi Write pour le wrapper lui-même
// (pour pouvoir écrire dans tous les flux d'un coup si besoin)
impl<W: Write> Write for MultiWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for w in &mut self.writers {
            w.write_all(buf)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        for w in &mut self.writers {
            w.flush()?;
        }
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdin_lock = stdin.lock();
    let args: Vec<String> = env::args().collect();
    traitement(stdin_lock, io::stdout(), args)
}

fn traitement<R: BufRead, W: Write>(
    source: R,
    mut destination: W,
    args: Vec<String>,
) -> io::Result<()> {
    // Gestion du flag -a (append)
    let append = args.iter().any(|a| a == "-a");
    let filenames: Vec<&String> = args[1..].iter().filter(|a| *a != "-a").collect();

    let flush_every = 50; // flush toutes les X lignes

    // Ouverture des fichiers
    let files: Vec<File> = filenames
        .iter()
        .map(|name| {
            OpenOptions::new()
                .write(true)
                .create(true)
                .append(append)
                .truncate(!append)
                .open(name)
                .expect(&format!("Impossible d'ouvrir le fichier : {}", name))
        })
        .collect();

    // === WRITER ===
    let writers: Vec<_> = files.into_iter()
        .map(|p| BufWriter::new(p))
        .collect();

    let mut writers = MultiWriter::new(writers);

    // === LECTURE STDIN ===
    let stdin = source;
    let mut reader = BufReader::new(stdin);
    let mut buffer = Vec::new();
    let mut counter = 0;

    while reader.read_until(b'\n', &mut buffer)? != 0 {
        // affichage console (safe UTF-8)
        write!(destination, "{}", String::from_utf8_lossy(&buffer))
            .expect("erreur pour ecrire la sortie");

        // envoi au writer thread
        writers.write_all(&buffer)?;
        if counter % flush_every == 0 {
            writers.flush()?;
        }

        buffer.clear();
        counter += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{TempDir, tempdir};

    #[test]
    fn test_nominal_avec_un_fichier_sans_append() {
        let mut parametres = Vec::new();
        parametres.push("mon_test.txt".to_string());
        test_simple("0123456789", parametres);
    }

    #[test]
    fn test_nominal_avec_un_fichier_sans_append_deux_fois() {
        {
            // 1er appel
            let mut parametres = Vec::new();
            parametres.push("mon_test.txt".to_string());
            test_simple("0123456789", parametres);
        }
        {
            // 2eme appel
            let mut parametres = Vec::new();
            parametres.push("mon_test.txt".to_string());
            test_simple("un test très simple", parametres);
        }
    }

    #[test]
    fn test_nominal_avec_deux_fichiers_sans_append() {
        let mut parametres = Vec::new();
        parametres.push("mon_test.txt".to_string());
        parametres.push("mon_test2.txt".to_string());
        test_simple("0123456789", parametres);
    }

    #[test]
    fn test_nominal_avec_un_fichier_sans_append_texte_plusieurs_lignes() {
        let message = "message 1\nmessage 2\nmessage 3";
        let mut parametres = Vec::new();
        parametres.push("mon_test.txt".to_string());
        test_simple(message, parametres);
    }

    #[test]
    fn test_nominal_avec_un_fichier_sans_append_texte_long() {
        let motif = "abcTest_Aa";
        let message = (1..=300) // Crée une plage de 1 à 300
            .map(|i| format!("{}{}", motif, i)) // Transforme chaque nombre en "abcX"
            .collect::<Vec<_>>() // Met tout dans un vecteur
            .join("-");
        let mut parametres = Vec::new();
        parametres.push("mon_test.txt".to_string());
        test_simple(message.as_str(), parametres);
    }

    #[test]
    fn test_nominal_avec_un_fichier_avec_append() {
        let message = "0123456789";
        let dir = tempdir().expect("Impossible de créer le dossier temp");
        let mut parametres = Vec::new();
        parametres.push("mon_test.txt".to_string());
        parametres.push("-a".to_string());
        test_simple_output(message, parametres, Some(message), &dir);
    }

    #[test]
    fn test_nominal_avec_un_fichier_avec_append_deux_fois() {
        let dir = tempdir().expect("Impossible de créer le dossier temp");
        {
            // 1er appel
            let message = "0123456789";
            let mut parametres = Vec::new();
            parametres.push("mon_test.txt".to_string());
            parametres.push("-a".to_string());
            test_simple_output(message, parametres, Some(message), &dir);
        }
        {
            // 2eme appel
            let message = "un test très simple";
            let message_out = "0123456789un test très simple";
            let mut parametres = Vec::new();
            parametres.push("mon_test.txt".to_string());
            parametres.push("-a".to_string());
            test_simple_output(message, parametres, Some(message_out), &dir);
        }
    }

    // méthodes utilitaires

    fn test_simple(message: &str, parametres: Vec<String>) {
        let dir = tempdir().expect("Impossible de créer le dossier temp");
        test_simple_output(message, parametres, None, &dir);
    }

    fn test_simple_output(
        message: &str,
        parametres: Vec<String>,
        output_file: Option<&str>,
        temp_dir: &TempDir,
    ) {
        // 2. Préparation de la sortie (Stdout simulé)
        let mut writer = Vec::new();

        let mut args: Vec<String> = Vec::new();

        // 1. Crée un répertoire temporaire
        // Il sera supprimé automatiquement à la fin de cette fonction
        let dir = temp_dir;
        args.push("executable.sh".to_string());
        for p in &parametres {
            if p == "-a" {
                args.push("-a".to_string());
            } else {
                let chemin_fichier = dir.path().join(p);
                args.push(chemin_fichier.to_str().unwrap().to_string());
            }
        }

        let mut bytes = message.as_bytes();

        // 3. Exécution
        let res = traitement(&mut bytes, &mut writer, args);

        // 4. Vérification
        let resultat = String::from_utf8(writer).expect("Sortie non valide UTF-8");
        assert_eq!(resultat, message);

        let mut message_out = message;
        if let Some(output_file) = output_file {
            message_out = output_file;
        }

        for p in parametres {
            if p != "-a" {
                let nom_fichier = dir.path().join(p);
                let contenu = fs::read_to_string(nom_fichier);
                assert_eq!(contenu.unwrap(), message_out);
            }
        }

        res.expect("Erreur de traitement");
    }
}
