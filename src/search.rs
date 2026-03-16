use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

pub struct SearchOptions<'a> {
    pub app_name: &'a str,
    pub exclude_list: &'a [String],
    pub deep: bool,
    pub case_sensitive: bool,
    pub exact: bool,
    pub max_depth: Option<usize>,
}

pub fn search(
    root: &Path,
    opts: &SearchOptions,
    progress_bar: &indicatif::ProgressBar,
) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let walker = if let Some(depth) = opts.max_depth {
        WalkDir::new(root).max_depth(depth)
    } else {
        WalkDir::new(root)
    };
    let mut it = walker.into_iter();
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

#[cfg(test)]
mod tests {
    use super::*;
    use indicatif::ProgressBar;
    use std::fs;

    fn hidden_progress_bar() -> ProgressBar {
        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        pb
    }

    fn default_opts(app_name: &str) -> SearchOptions<'_> {
        SearchOptions {
            app_name,
            exclude_list: &[],
            deep: false,
            case_sensitive: false,
            exact: false,
            max_depth: None,
        }
    }

    #[test]
    fn finds_matching_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Spotify.plist"), "data").unwrap();
        fs::write(tmp.path().join("unrelated.txt"), "data").unwrap();

        let pb = hidden_progress_bar();
        let results = search(tmp.path(), &default_opts("spotify"), &pb);

        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("Spotify.plist"));
    }

    #[test]
    fn finds_matching_directory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir(tmp.path().join("com.spotify.client")).unwrap();
        fs::write(
            tmp.path().join("com.spotify.client").join("inner.txt"),
            "data",
        )
        .unwrap();

        let pb = hidden_progress_bar();
        let results = search(tmp.path(), &default_opts("spotify"), &pb);

        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("com.spotify.client"));
    }

    #[test]
    fn case_insensitive_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("SPOTIFY.plist"), "data").unwrap();
        fs::write(tmp.path().join("spotify.cache"), "data").unwrap();

        let pb = hidden_progress_bar();
        let results = search(tmp.path(), &default_opts("Spotify"), &pb);

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn case_sensitive_search() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("Spotify.plist"), "data").unwrap();
        fs::write(tmp.path().join("spotify.cache"), "data").unwrap();

        let pb = hidden_progress_bar();
        let opts = SearchOptions {
            case_sensitive: true,
            ..default_opts("Spotify")
        };
        let results = search(tmp.path(), &opts, &pb);

        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("Spotify.plist"));
    }

    #[test]
    fn exact_match() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("chrome.plist"), "data").unwrap();
        fs::write(tmp.path().join("chromium.plist"), "data").unwrap();
        fs::write(tmp.path().join("com.google.chrome.plist"), "data").unwrap();

        let pb = hidden_progress_bar();
        let opts = SearchOptions {
            exact: true,
            ..default_opts("chrome")
        };
        let results = search(tmp.path(), &opts, &pb);

        assert_eq!(results.len(), 2);
        let names: Vec<_> = results.iter().map(|p| p.file_name().unwrap()).collect();
        assert!(names.contains(&std::ffi::OsStr::new("chrome.plist")));
        assert!(names.contains(&std::ffi::OsStr::new("com.google.chrome.plist")));
    }

    #[test]
    fn excludes_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let keep = tmp.path().join("keepdir");
        fs::create_dir(&keep).unwrap();
        fs::write(keep.join("spotify.plist"), "data").unwrap();
        fs::write(tmp.path().join("spotify.cache"), "data").unwrap();

        let pb = hidden_progress_bar();
        let exclude = vec!["keepdir".to_string()];
        let opts = SearchOptions {
            exclude_list: &exclude,
            ..default_opts("spotify")
        };
        let results = search(tmp.path(), &opts, &pb);

        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("spotify.cache"));
    }

    #[test]
    fn max_depth_limits_search() {
        let tmp = tempfile::tempdir().unwrap();
        // depth 1: direct child
        fs::write(tmp.path().join("spotify.txt"), "data").unwrap();
        // depth 2: nested
        let nested = tmp.path().join("sub");
        fs::create_dir(&nested).unwrap();
        fs::write(nested.join("spotify.log"), "data").unwrap();

        let pb = hidden_progress_bar();
        let opts = SearchOptions {
            max_depth: Some(1),
            ..default_opts("spotify")
        };
        let results = search(tmp.path(), &opts, &pb);

        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("spotify.txt"));
    }

    #[test]
    fn no_results_when_nothing_matches() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("unrelated.txt"), "data").unwrap();

        let pb = hidden_progress_bar();
        let results = search(tmp.path(), &default_opts("spotify"), &pb);

        assert!(results.is_empty());
    }

    #[test]
    fn skips_dir_contents_after_match() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("spotify_data");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("spotify_inner.txt"), "data").unwrap();

        let pb = hidden_progress_bar();
        let results = search(tmp.path(), &default_opts("spotify"), &pb);

        // Should only return the directory, not the inner file
        assert_eq!(results.len(), 1);
        assert!(results[0].ends_with("spotify_data"));
    }
}
