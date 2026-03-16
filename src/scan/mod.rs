#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::run_scan;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::run_scan;

#[cfg(not(any(target_os = "macos", windows)))]
pub fn run_scan(_min_confidence: f32, _min_threshold: Option<f32>) -> crate::Result<()> {
    eprintln!("scan is not supported on Linux yet — coming soon");
    std::process::exit(1);
}

#[cfg(any(target_os = "macos", windows))]
pub struct OrphanCandidate {
    pub path: std::path::PathBuf,
    pub size: u64,
    pub confidence: f32,
}

pub(crate) fn entry_size(path: &std::path::Path) -> u64 {
    if path.is_file() {
        path.metadata().map(|m| m.len()).unwrap_or(0)
    } else {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| e.metadata().ok().map(|m| m.len()))
            .sum()
    }
}

// ── Shared constants and helpers used by platform modules ──

#[cfg(any(target_os = "macos", windows))]
pub(crate) const NOISE_TOKENS: &[&str] = &["com", "org", "net", "app", "io", "the", "get"];
#[cfg(any(target_os = "macos", windows))]
pub(crate) const NAME_SEPARATORS: &[char] = &['.', '-', '_', ' '];

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn tokenize_name(name: &str) -> Vec<String> {
    name.split(|c: char| NAME_SEPARATORS.contains(&c))
        .map(|t| t.to_lowercase())
        .filter(|t| !t.is_empty() && !NOISE_TOKENS.contains(&t.as_str()))
        .collect()
}

#[cfg(any(target_os = "macos", windows))]
pub(crate) fn build_tokens(name: &str, extra: &str) -> Vec<String> {
    let mut tokens = tokenize_name(name);
    if !extra.is_empty() {
        tokens.extend(tokenize_name(extra));
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Token overlap ratio: proportion of `folder_tokens` found in `app_tokens`.
#[cfg(any(target_os = "macos", windows))]
pub(crate) fn token_overlap(folder_tokens: &[String], app_tokens: &[String]) -> f32 {
    let common = folder_tokens
        .iter()
        .filter(|t| app_tokens.contains(t))
        .count();
    common as f32 / folder_tokens.len().max(1) as f32
}

/// Base confidence from 3 platform-agnostic signals.
/// Returns a partial score; callers may add platform-specific signals before clamping.
#[cfg(any(target_os = "macos", windows))]
pub(crate) fn base_confidence(path: &std::path::Path, size: u64, best_match_score: f32) -> f32 {
    let mut score = 0.0f32;

    // Signal 1 — inverse quality of best app match (0.0 to 0.35)
    score += match best_match_score {
        0.0 => 0.35,
        s if s < 0.15 => 0.25,
        s if s < 0.3 => 0.15,
        s if s < 0.45 => 0.05,
        _ => 0.0,
    };

    // Signal 2 — age via mtime (-0.3 to +0.35)
    if let Ok(metadata) = path.metadata()
        && let Ok(modified) = metadata.modified()
        && let Ok(age) = modified.elapsed()
    {
        let days = age.as_secs() / 86400;
        score += match days {
            d if d < 7 => -0.3,
            d if d < 30 => -0.15,
            d if d < 90 => 0.0,
            d if d < 180 => 0.1,
            d if d < 365 => 0.2,
            _ => 0.35,
        };
    }

    // Signal 3 — size (-0.15 to +0.2)
    score += match size {
        0 => 0.2,
        1..=1023 => 0.1,
        1024..=1_048_576 => 0.0,
        s if s <= 100 * 1024 * 1024 => -0.05,
        _ => -0.15,
    };

    score
}

/// Shared UX: filter, display MultiSelect, confirm, delete.
#[cfg(any(target_os = "macos", windows))]
pub(crate) fn present_orphans(
    orphans: Vec<OrphanCandidate>,
    min_confidence: f32,
    min_threshold: Option<f32>,
) -> crate::Result<()> {
    use dialoguer::{MultiSelect, Select, theme::ColorfulTheme};

    let orphans: Vec<_> = if let Some(threshold) = min_threshold {
        orphans
            .into_iter()
            .filter(|o| o.confidence >= threshold)
            .collect()
    } else {
        orphans
    };

    if orphans.is_empty() {
        println!("No orphan files or directories found.");
        return Ok(());
    }

    let theme = ColorfulTheme::default();

    let labels: Vec<String> = orphans
        .iter()
        .map(|o| {
            format!(
                "{} ({}, confidence: {:.0}%)",
                o.path.display(),
                crate::format_size(o.size),
                o.confidence * 100.0
            )
        })
        .collect();

    let defaults: Vec<bool> = orphans
        .iter()
        .map(|o| o.confidence >= min_confidence)
        .collect();

    let Some(selections) = MultiSelect::with_theme(&theme)
        .with_prompt("Select items to delete (Space to toggle, Enter to confirm, Esc to exit)")
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

    let selected_orphans: Vec<&OrphanCandidate> = selections.iter().map(|&i| &orphans[i]).collect();
    let selected_size: u64 = selected_orphans.iter().map(|o| o.size).sum();

    let Some(confirm) = Select::with_theme(&theme)
        .with_prompt(format!(
            "About to delete {} items ({}). This cannot be undone. Confirm?",
            selected_orphans.len(),
            crate::format_size(selected_size)
        ))
        .item("Yes, delete selected items")
        .item("No, go back")
        .default(1)
        .interact_opt()?
    else {
        return Ok(());
    };

    if confirm == 0 {
        let paths: Vec<std::path::PathBuf> =
            selected_orphans.iter().map(|o| o.path.clone()).collect();
        crate::clean_files::clean_files(paths);
        println!("Done.");
    }

    Ok(())
}
