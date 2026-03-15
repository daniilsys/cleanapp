use clap::Parser;
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
    about = "Clean application files and directories on macOS",
    before_help = "Use sudo to run this tool if you want to clean files in /Library. For example: sudo cleanapp MyApp"
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
}

fn main() -> Result<()> {
    let args = Args::parse();

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
    };

    let results = get_results(&options, &spinner)?;
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
                            println!("\x1b[32m[FILE]\x1b[0m {}", path.display()); // vert
                        } else if path.is_dir() {
                            println!("\x1b[34m[DIR]\x1b[0m  {}", path.display()); // bleu
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
