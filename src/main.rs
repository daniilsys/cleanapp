use clap::Parser;
use std::env;
use std::fs;
use std::io::stdin;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "cleanapp",
    about = "Clean application files and directories on macOS",
    before_help = "Use sudo to run this tool if you want to clean files in /Library. For example: sudo cleanapp --list MyApp"
)]
struct Args {
    #[arg(help = "Name of the app to clean or list files and directories for")]
    app_name: String,
    #[arg(long)]
    #[arg(help = "Whether to list files and directories instead of cleaning them")]
    list: bool,
    #[arg(long)]
    #[arg(help = "list of file or directory names to exclude from cleaning")]
    exclude: Vec<String>,
}

fn main() {
    let args = Args::parse();

    let Some(home_env) = env::var_os("HOME") else {
        return eprintln!("Error: HOME environment variable not set");
    };
    let home = PathBuf::from(home_env);

    let library = home.join("Library");
    let sys_library = "/Library";

    let mut results = Vec::new();
    let app_name_lower = args.app_name.to_ascii_lowercase();
    let exclude_list = args
        .exclude
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect::<Vec<_>>();

    results.extend(search_library(&library, &app_name_lower, &exclude_list));
    results.extend(search_library(
        Path::new(sys_library),
        &app_name_lower,
        &exclude_list,
    ));
    if results.is_empty() {
        println!("No files or directories found for app: {}", args.app_name);
        return;
    }

    if args.list {
        println!("Listing files for app: {}", args.app_name);
        for path in &results {
            if path.is_file() {
                println!("\x1b[32m[FILE]\x1b[0m {}", path.display()); // vert
            } else if path.is_dir() {
                println!("\x1b[34m[DIR]\x1b[0m  {}", path.display()); // bleu
            }
        }
    }

    println!("\x1b[34m[!!!] Note: deleting from /Library may require sudo\x1b[0m");
    println!(
        "\x1b[33m[!!!] Are you sure you want to clean \x1b[1m{}\x1b[0m \x1b[33mfiles and directories? (y/n)\x1b[0m",
        results.len()
    );

    let mut input = String::new();
    stdin().read_line(&mut input).expect("Failed to read input");
    let input = input.trim().to_ascii_lowercase();
    if input == "yes" || input == "y" {
        clean_files(results);
    } else {
        println!("Aborting cleaning process.");
    }
}

fn search_library(root: &Path, app_name: &str, exclude_list: &[String]) -> Vec<PathBuf> {
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

fn clean_files(paths: Vec<PathBuf>) {
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
