use clap::{Parser, Subcommand};
use console::style;
use dialoguer::{MultiSelect, Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use search::SearchOptions;
use std::path::PathBuf;
use std::time::Duration;
mod clean_files;
use clean_files::clean_files;
mod get_results;
use get_results::get_results;
mod scan;
mod search;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Parser)]
#[command(
    name = "cleanapp",
    about = "Find and clean leftover application files and directories",
    before_help = "Run with elevated privileges (sudo on macOS/Linux, Administrator on Windows) to clean system-level files."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find and clean leftover files for a specific application
    Clean {
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
    },
    /// Scan for orphan application files (macOS, Linux & Windows)
    Scan {
        #[arg(
            long,
            default_value = "0.5",
            value_parser = parse_confidence,
            help = "Pre-select orphans with confidence >= this threshold (0.0 to 1.0)"
        )]
        confidence: f32,
        #[arg(
            long,
            value_parser = parse_confidence,
            help = "Only show orphans with confidence >= this value (0.0 to 1.0)"
        )]
        atleast: Option<f32>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if !is_elevated() {
        eprintln!("{}",
            style("[WARNING] You are running this tool without elevated privileges. Some files and directories may not be found or cleaned.").yellow()
        );
    }

    match cli.command {
        Commands::Clean {
            app_name,
            exclude,
            deep,
            case_sensitive,
            exact,
            here,
            max_depth,
            add,
        } => run_clean(
            app_name,
            exclude,
            deep,
            case_sensitive,
            exact,
            here,
            max_depth,
            add,
        ),
        Commands::Scan {
            confidence,
            atleast,
        } => scan::run_scan(confidence, atleast),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_clean(
    app_name: String,
    exclude: Vec<String>,
    deep: bool,
    case_sensitive: bool,
    exact: bool,
    here: bool,
    max_depth: Option<usize>,
    add: Vec<PathBuf>,
) -> Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_message(format!(
        "Searching for files and directories related to '{}'",
        app_name
    ));
    spinner.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_strings(&["-", "\\", "|", "/"]),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));

    let options = SearchOptions {
        app_name: &app_name,
        exclude_list: &exclude,
        deep,
        case_sensitive,
        exact,
        max_depth,
    };

    let results = get_results(&options, &spinner, here, &add)?;
    spinner.finish_and_clear();

    if results.is_empty() {
        println!("No files or directories found for app: {}", app_name);
        return Ok(());
    }
    let theme = ColorfulTheme::default();

    let labels: Vec<String> = results
        .iter()
        .map(|p| {
            let size = scan::entry_size(p);
            format!("{} ({})", p.display(), format_size(size))
        })
        .collect();

    let defaults: Vec<bool> = vec![true; results.len()];

    let Some(selections) = MultiSelect::with_theme(&theme)
        .with_prompt(
            "Select items to delete (Space to toggle, Enter to confirm, 'a' to toggle all)",
        )
        .items(&labels)
        .defaults(&defaults)
        .interact_opt()?
    else {
        return Ok(());
    };

    if selections.is_empty() {
        println!("Nothing selected. Exiting.");
        return Ok(());
    }

    let selected: Vec<PathBuf> = selections.iter().map(|&i| results[i].clone()).collect();
    let selected_size = total_size(&selected);

    let Some(confirm) = Select::with_theme(&theme)
        .with_prompt(format!(
            "About to delete {} items ({}). This cannot be undone. Confirm?",
            selected.len(),
            format_size(selected_size)
        ))
        .item("Yes, delete selected items")
        .item("No, go back")
        .default(1)
        .interact_opt()?
    else {
        return Ok(());
    };

    if confirm == 0 {
        clean_files(selected);
        println!("Done.");
    }

    Ok(())
}

fn total_size(paths: &[PathBuf]) -> u64 {
    paths.iter().map(|p| scan::entry_size(p)).sum()
}

pub fn format_size(size: u64) -> String {
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

fn parse_confidence(s: &str) -> std::result::Result<f32, String> {
    let val: f32 = s.parse().map_err(|e| format!("{e}"))?;
    if (0.0..=1.0).contains(&val) {
        Ok(val)
    } else {
        Err("confidence must be between 0.0 and 1.0".to_string())
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
