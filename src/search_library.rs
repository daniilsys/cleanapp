use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn search_library(root: &Path, app_name: &str, exclude_list: &[String]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let mut it = WalkDir::new(root).into_iter();

    loop {
        let entry = match it.next() {
            None => break,
            Some(Ok(e)) => e,
            Some(Err(_)) => continue,
        };
        let path_lower = entry
            .path()
            .as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase();

        if let Some(file_name) = entry.file_name().to_ascii_lowercase().to_str() {
            if file_name.contains(app_name)
                && !exclude_list
                    .iter()
                    .any(|ex| path_lower.contains(ex.as_str()))
            {
                if entry.file_type().is_dir() {
                    results.push(entry.path().to_path_buf());
                    it.skip_current_dir();
                    continue;
                } else if entry.file_type().is_file() {
                    results.push(entry.path().to_path_buf());
                }
            }
        }
    }
    results
}
