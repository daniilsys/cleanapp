use indicatif::ProgressBar;

use crate::Result;
use crate::search::{SearchOptions, search};
use dirs::{cache_dir, config_dir, data_dir, home_dir};
use std::path::PathBuf;

pub fn get_results(
    opts: &SearchOptions,
    progress_bar: &ProgressBar,
    here: bool,
    extra_paths: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let mut results = Vec::new();
    let app_name_lower = opts.app_name.to_ascii_lowercase();
    let exclude_list = opts
        .exclude_list
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>();

    let options = &SearchOptions {
        app_name: &app_name_lower,
        exclude_list: &exclude_list,
        deep: opts.deep,
        case_sensitive: opts.case_sensitive,
        exact: opts.exact,
        max_depth: opts.max_depth,
    };

    let mut roots = if here {
        vec![std::env::current_dir()?]
    } else {
        let home_dir = home_dir().ok_or("Could not determine home directory")?;
        get_roots(opts.deep, home_dir)?
    };

    for path in extra_paths {
        if !roots.contains(path) {
            roots.push(path.clone());
        }
    }

    for root in roots {
        let mut found = search(&root, options, progress_bar);
        results.append(&mut found);
    }

    Ok(results)
}

fn get_roots(deep: bool, home_dir: PathBuf) -> Result<Vec<PathBuf>> {
    if deep {
        return Ok(vec![home_dir]);
    }

    let mut roots = Vec::new();

    if cfg!(target_os = "macos") {
        Ok(vec![home_dir.join("Library"), PathBuf::from("/Library")])
    } else {
        if let Some(dir) = cache_dir() {
            roots.push(dir);
        }
        if let Some(dir) = config_dir() {
            roots.push(dir);
        }
        if let Some(dir) = data_dir()
            && !roots.contains(&dir)
        {
            roots.push(dir);
        }

        Ok(roots)
    }
}
