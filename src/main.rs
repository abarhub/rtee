use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

use std::io::BufWriter;
use std::panic;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

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
    // let args: Vec<String> = env::args().collect();

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

    // === CHANNEL ===
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    // === WRITER THREAD ===
    let writers: Vec<_> = files
        .into_iter()
        //.map(|p| BufWriter::new(File::create(p).unwrap()))
        .map(|p| BufWriter::new(p))
        .collect();

    let writers = Arc::new(Mutex::new(writers));

    // Hook panic pour flush
    {
        let writers = Arc::clone(&writers);
        panic::set_hook(Box::new(move |_| {
            eprintln!("⚠️ Panic détecté, flush en cours...");
            if let Ok(mut ws) = writers.lock() {
                for w in ws.iter_mut() {
                    let _ = w.flush();
                }
            }
        }));
    }

    let writer_handle = {
        let writers = Arc::clone(&writers);

        thread::spawn(move || {
            let mut counter = 0;

            while let Ok(buffer) = rx.recv() {
                let mut ws = writers.lock().unwrap();

                for w in ws.iter_mut() {
                    let _ = w.write_all(&buffer);
                }

                counter += 1;

                // flush périodique
                if counter % flush_every == 0 {
                    for w in ws.iter_mut() {
                        let _ = w.flush();
                    }
                }
            }

            // flush final
            let mut ws = writers.lock().unwrap();
            for w in ws.iter_mut() {
                let _ = w.flush();
            }
        })
    };

    // === LECTURE STDIN ===
    let stdin = source;
    let mut reader = BufReader::new(stdin);
    let mut buffer = Vec::new();

    while reader.read_until(b'\n', &mut buffer)? != 0 {
        // affichage console (safe UTF-8)
        write!(destination, "{}", String::from_utf8_lossy(&buffer))
            .expect("erreur pour ecrire la sortie");

        // envoi au writer thread
        tx.send(buffer.clone()).unwrap();

        buffer.clear();
    }

    // fermeture channel → stop writer thread
    drop(tx);

    // attendre la fin
    let _ = writer_handle.join();

    println!("Fin du traitement");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    // Importe ma_fonction
    //     use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_ma_fonction_transformation() {
        // 1. Préparation de l'entrée (Stdin simulé)
        //let entree_simulee = "Hello Rust\n";
        //let reader = Cursor::new(entree_simulee);

        // 2. Préparation de la sortie (Stdout simulé)
        let mut writer = Vec::new();

        let mut args: Vec<String> = Vec::new();

        // 1. Crée un répertoire temporaire
        // Il sera supprimé automatiquement à la fin de cette fonction
        let dir = tempdir().expect("Impossible de créer le dossier temp");
        let chemin_fichier = dir.path().join("mon_test.txt");
        args.push(chemin_fichier.to_str().unwrap().to_string());

        let mut bytes = &b"0123456789"[..];

        // 3. Exécution
        let res = traitement(&mut bytes, &mut writer, args);

        // 4. Vérification
        let resultat = String::from_utf8(writer).expect("Sortie non valide UTF-8");
        assert_eq!(resultat, "0123456789");

        // let nom_fichier = chemin_fichier.to_str().unwrap();
        // println!("fichier {}", nom_fichier);
        // let contenu = fs::read_to_string(nom_fichier);
        // assert_eq!(contenu.unwrap(), "0123456789");

        res.expect("Erreur de traitement");
    }
}
