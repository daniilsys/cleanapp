use indicatif::ProgressBar;

use crate::Result;
use crate::search::{SearchOptions, search};
use std::env;
use std::path::{Path, PathBuf};

pub fn get_results(opts: &SearchOptions, progress_bar: &ProgressBar) -> Result<Vec<PathBuf>> {
    let Some(home_env) = env::var_os("HOME") else {
        return Err(
            "HOME environment variable is not set. Please run this tool in a terminal with a valid user session.".into()
        );
    };

    let home = PathBuf::from(home_env);

    let library = home.join("Library");
    let sys_library = "/Library";

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
    };

    if opts.deep {
        results.extend(search(&home, options, progress_bar));
    } else {
        results.extend(search(&library, options, progress_bar));
        results.extend(search(Path::new(sys_library), options, progress_bar));
    }
    Ok(results)
}
