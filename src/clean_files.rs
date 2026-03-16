use console::style;
use std::fs;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn removes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.txt");
        fs::write(&file, "data").unwrap();
        assert!(file.exists());

        clean_files(vec![file.clone()]);

        assert!(!file.exists());
    }

    #[test]
    fn removes_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("testdir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("inner.txt"), "data").unwrap();
        assert!(dir.exists());

        clean_files(vec![dir.clone()]);

        assert!(!dir.exists());
    }

    #[test]
    fn handles_nonexistent_path() {
        let path = PathBuf::from("/tmp/cleanapp_nonexistent_path_test");
        // Should not panic
        clean_files(vec![path]);
    }
}

pub fn clean_files(paths: Vec<PathBuf>) {
    for path in paths {
        if !path.exists() {
            eprintln!(
                "{} Path does not exist: {}",
                style("[ERROR]").red(),
                path.display()
            );
            continue;
        }

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
                "{} Removed {}: {}",
                style("[OK]").green(),
                dir_or_file,
                path.display()
            ),
            Err(e) => eprintln!(
                "{} Failed to remove {}: {}",
                style("[ERROR]").red(),
                path.display(),
                e
            ),
        }
    }
}
