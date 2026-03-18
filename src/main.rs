use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    // Gestion du flag -a (append)
    let append = args.iter().any(|a| a == "-a");
    let filenames: Vec<&String> = args[1..]
        .iter()
        .filter(|a| *a != "-a")
        .collect();

    // Ouverture des fichiers
    let mut files: Vec<File> = filenames
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

    // Lecture de stdin ligne par ligne
    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line?;
        let output = format!("{}\n", line);

        // Écriture sur stdout
        print!("{}", output);

        // Écriture dans chaque fichier
        for file in &mut files {
            file.write_all(output.as_bytes())?;
        }
    }

    Ok(())
}