use std::fs;
use std::path::PathBuf;

pub fn clean_files(paths: Vec<PathBuf>) {
    for path in paths {
        let dir_or_file = if path.is_file() { "file" } else { "directory" };

        let result = if path.is_file() {
            fs::remove_file(&path)
        } else if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            continue;
        };

        match result {
            Ok(_) => println!(
                "\x1b[32m[OK]\x1b[0m Removed {}: {}",
                dir_or_file,
                path.display()
            ),
            Err(e) => eprintln!(
                "\x1b[31m[ERROR]\x1b[0m Failed to remove {}: {}",
                path.display(),
                e
            ),
        }
    }
}
