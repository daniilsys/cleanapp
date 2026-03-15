use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

pub struct SearchOptions<'a> {
    pub app_name: &'a str,
    pub exclude_list: &'a [String],
    pub deep: bool,
    pub case_sensitive: bool,
    pub exact: bool,
}

pub fn search(
    root: &Path,
    opts: &SearchOptions,
    progress_bar: &indicatif::ProgressBar,
) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let mut it = WalkDir::new(root).into_iter();
    let mut last_update = Instant::now();

    let app_name = if opts.case_sensitive {
        opts.app_name.to_string()
    } else {
        opts.app_name.to_ascii_lowercase()
    };

    loop {
        let entry = match it.next() {
            None => break,
            Some(Ok(e)) => e,
            Some(Err(_)) => continue,
        };
        let mut path = entry.path().as_os_str().to_string_lossy();

        if let Some(file_name) = entry.file_name().to_str() {
            if !opts.case_sensitive {
                path = path.to_ascii_lowercase().into();
            }

            let file_name = if opts.case_sensitive {
                file_name.to_string()
            } else {
                file_name.to_ascii_lowercase()
            };

            if last_update.elapsed().as_millis() >= 500 {
                progress_bar.set_message(format!("Scanning: {}", entry.path().display()));
                last_update = Instant::now();
            }

            if opts
                .exclude_list
                .iter()
                .any(|ex| path.contains(ex.as_str()))
            {
                continue;
            }

            let matched = if opts.exact {
                let separators = [' ', '-', '_', '.'];
                file_name
                    .split(|c| separators.contains(&c))
                    .any(|part| part == app_name)
            } else {
                file_name.contains(&*app_name)
            };

            if matched {
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
