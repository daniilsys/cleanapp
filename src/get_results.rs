use crate::Result;
use crate::search_library::search_library;
use std::env;
use std::path::{Path, PathBuf};

pub fn get_results(app_name: &str, exclude_list: &[String]) -> Result<Vec<PathBuf>> {
    let Some(home_env) = env::var_os("HOME") else {
        return Err(
            "HOME environment variable is not set. Please run this tool in a terminal with a valid user session.".into()
        );
    };

    let home = PathBuf::from(home_env);

    let library = home.join("Library");
    let sys_library = "/Library";

    let mut results = Vec::new();
    let app_name_lower = app_name.to_ascii_lowercase();
    let exclude_list = exclude_list
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>();

    results.extend(search_library(&library, &app_name_lower, &exclude_list));
    results.extend(search_library(
        Path::new(sys_library),
        &app_name_lower,
        &exclude_list,
    ));
    Ok(results)
}
