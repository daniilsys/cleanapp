use clap::Parser;
use console::style;
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use search::SearchOptions;
use std::path::PathBuf;
use std::time::Duration;
mod clean_files;
use clean_files::clean_files;
mod get_results;
use get_results::get_results;
use walkdir::WalkDir;
mod search;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "cleanapp",
    about = "Find and clean leftover application files and directories",
    before_help = "Run with elevated privileges (sudo on macOS/Linux, Administrator on Windows) to clean system-level files."
)]
struct Args {
    #[arg(help = "Name of the app to clean or list files and directories for")]
    app_name: String,
    #[arg(long)]
    #[arg(help = "list of file or directory names to exclude from cleaning")]
    exclude: Vec<String>,
    #[arg(long)]
    #[arg(
        help = "Make a deep search for files and directories related to the app. This may take more time but will find more items."
    )]
    deep: bool,
    #[arg(long)]
    #[arg(help = "Case sensitive search. By default, the search is case insensitive.")]
    case_sensitive: bool,
    #[arg(long)]
    #[arg(help = "Exact match search. By default, the search is a substring match.")]
    exact: bool,
    #[arg(long)]
    #[arg(help = "Search only in the current directory and its subdirectories")]
    here: bool,
    #[arg(long)]
    #[arg(help = "Maximum depth of subdirectories to search")]
    max_depth: Option<usize>,
    #[arg(long)]
    #[arg(help = "Add a custom path to search in (repeatable)")]
    add: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if !is_elevated() {
        eprintln!("{}",
            style("[WARNING] You are running this tool without elevated privileges. Some files and directories may not be found or cleaned.").yellow()
        );
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_message(format!(
        "Searching for files and directories related to '{}'",
        args.app_name
    ));
    spinner.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_strings(&["-", "\\", "|", "/"]),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));

    let options = SearchOptions {
        app_name: &args.app_name,
        exclude_list: &args.exclude,
        deep: args.deep,
        case_sensitive: args.case_sensitive,
        exact: args.exact,
        max_depth: args.max_depth,
    };

    let results = get_results(&options, &spinner, args.here, &args.add)?;
    spinner.finish_and_clear();

    if results.is_empty() {
        println!("No files or directories found for app: {}", args.app_name);
        return Ok(());
    }
    let theme = ColorfulTheme::default();
    let size = total_size(&results);
    let formated_size = format_size(size);

    loop {
        let select = Select::with_theme(&theme)
            .with_prompt(format!(
                "Found {} items for app '{}'. Total size: {}.\nWhat do you want to do?",
                results.len(),
                args.app_name,
                formated_size
            ))
            .item("List all found files and directories")
            .item("Clean all found files and directories")
            .item("Exit")
            .default(0);

        if let Ok(choice) = select.interact() {
            match choice {
                0 => {
                    println!("Found items:");
                    for path in &results {
                        if path.is_file() {
                            println!("{} {}", style("[FILE]").green(), path.display()); // vert
                        } else if path.is_dir() {
                            println!("{} {}", style("[DIR]").blue(), path.display()); // bleu
                        }
                    }
                }
                1 => {
                    let confirm = Select::with_theme(&theme)
                        .with_prompt(
                            "Are you sure you want to clean all found files and directories?",
                        )
                        .item("Yes, clean them")
                        .item("No, go back")
                        .default(0);

                    if let Ok(confirm_choice) = confirm.interact() {
                        if confirm_choice == 0 {
                            clean_files(results);
                            break;
                        } else {
                            println!("Cleaning cancelled. Returning to main menu.");
                        }
                    } else {
                        eprintln!("Error reading user input. Returning to main menu.");
                    }
                }
                2 => {
                    println!("Exiting...");
                    break;
                }
                _ => unreachable!(),
            }
        } else {
            eprintln!("Error reading user input. Exiting...");
            break;
        }
    }
    Ok(())
}

fn total_size(paths: &[PathBuf]) -> u64 {
    let mut total = 0;
    for path in paths {
        if path.is_file() {
            if let Ok(metadata) = path.metadata() {
                total += metadata.len();
            }
        } else {
            total += WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.metadata().ok().map(|m| m.len()))
                .sum::<u64>();
        }
    }
    total
}

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes() {
        assert_eq!(format_size(0), "0 bytes");
        assert_eq!(format_size(512), "512 bytes");
    }

    #[test]
    fn format_kilobytes() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
    }

    #[test]
    fn format_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.00 MB");
    }

    #[test]
    fn format_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(3 * 1024 * 1024 * 1024), "3.00 GB");
    }
}

fn is_elevated() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    {
        std::fs::read_dir("C:\\Windows\\System32").is_ok()
    }
}
