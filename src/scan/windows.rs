use std::collections::HashMap;
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use winreg::RegKey;
use winreg::enums::*;

use super::OrphanCandidate;
use super::{base_confidence, build_tokens, token_overlap, tokenize_name};

const UNINSTALL_PATHS: &[(&str, bool)] = &[
    (
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        false,
    ),
    (r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", true), // HKCU
    (
        r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        false,
    ),
];

const EXCLUDED_NAMES: &[&str] = &[
    // Windows system components
    "Microsoft",
    "Windows",
    "WindowsApps",
    "Package Cache",
    "Packages",
    "SystemApps",
    "INetCache",
    "INetCookies",
    "History",
    "Temporary Internet Files",
    "Microsoft Edge",
    "MicrosoftEdge",
    // Runtime / SDK / dev tools
    "pip",
    "pnpm",
    "npm",
    "node-gyp",
    "typescript",
    "yarn",
    "cargo",
    "rustup",
    "NuGet",
    "dotnet",
    ".dotnet",
    "Python",
    "node_modules",
    // System-level caches
    "CrashDumps",
    "Temp",
    "D3DSCache",
    "FontCache",
    "IconCache",
    "ElevatedDiagnostics",
    "ConnectedDevicesPlatform",
];

/// Prefixes that indicate Windows system entries (lowercased for comparison)
const SYSTEM_NAME_PREFIXES: &[&str] = &[
    "microsoft.",
    "microsoft ",
    "windows ",
    "windows.",
    "{", // GUIDs like {12345-...}
];

struct InstalledApp {
    name: String,
    install_location: Option<PathBuf>,
    tokens: Vec<String>,
}

pub fn run_scan(min_confidence: f32, min_threshold: Option<f32>) -> crate::Result<()> {
    use std::time::Duration;

    let spinner = ProgressBar::new_spinner();
    spinner.set_message("Scanning installed applications...");
    spinner.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_strings(&["-", "\\", "|", "/"]),
    );
    spinner.enable_steady_tick(Duration::from_millis(100));

    let apps = discover_installed_apps(&spinner);
    spinner.set_message(format!(
        "Found {} installed apps. Scanning for orphans...",
        apps.len()
    ));

    let orphans = find_orphans(&apps, &spinner);
    spinner.finish_and_clear();

    super::present_orphans(orphans, min_confidence, min_threshold)
}

fn discover_installed_apps(spinner: &ProgressBar) -> Vec<InstalledApp> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut apps: Vec<InstalledApp> = Vec::new();

    for &(subkey_path, is_hkcu) in UNINSTALL_PATHS {
        let hive = if is_hkcu {
            RegKey::predef(HKEY_CURRENT_USER)
        } else {
            RegKey::predef(HKEY_LOCAL_MACHINE)
        };

        let Ok(uninstall_key) = hive.open_subkey_with_flags(subkey_path, KEY_READ) else {
            continue;
        };

        for name in uninstall_key.enum_keys().filter_map(|k| k.ok()) {
            let Ok(subkey) = uninstall_key.open_subkey_with_flags(&name, KEY_READ) else {
                continue;
            };

            let display_name: String = match subkey.get_value("DisplayName") {
                Ok(v) => v,
                Err(_) => continue,
            };

            if display_name.is_empty() {
                continue;
            }

            // Deduplicate by display name (case-insensitive)
            let key = display_name.to_lowercase();
            if seen.contains_key(&key) {
                // Merge: if this entry has install_location and existing doesn't, update it
                if let Ok(loc) = subkey.get_value::<String, _>("InstallLocation") {
                    if !loc.is_empty() {
                        let idx = seen[&key];
                        if apps[idx].install_location.is_none() {
                            apps[idx].install_location = Some(PathBuf::from(&loc));
                        }
                    }
                }
                continue;
            }

            let publisher: String = subkey.get_value("Publisher").unwrap_or_default();

            let install_location: Option<PathBuf> = subkey
                .get_value::<String, _>("InstallLocation")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from);

            let tokens = build_tokens(&display_name, &publisher);

            spinner.set_message(format!("Found: {display_name}"));

            seen.insert(key, apps.len());
            apps.push(InstalledApp {
                name: display_name,
                install_location,
                tokens,
            });
        }
    }

    // Discover Scoop apps if present
    if let Some(home) = dirs::home_dir() {
        let scoop_apps = home.join("scoop").join("apps");
        if scoop_apps.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&scoop_apps) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    let name = match path.file_name().and_then(|n| n.to_str()) {
                        Some(n) if n != "scoop" => n.to_string(),
                        _ => continue,
                    };

                    let key = name.to_lowercase();
                    if seen.contains_key(&key) {
                        continue;
                    }

                    let tokens = build_tokens(&name, "");
                    spinner.set_message(format!("Found (scoop): {name}"));

                    seen.insert(key, apps.len());
                    apps.push(InstalledApp {
                        name,
                        install_location: Some(path),
                        tokens,
                    });
                }
            }
        }
    }

    apps
}

fn correlate(folder_name: &str, folder_tokens: &[String], app: &InstalledApp) -> f32 {
    // Exact name match (case-insensitive)
    if folder_name.eq_ignore_ascii_case(&app.name) {
        return 1.0;
    }
    token_overlap(folder_tokens, &app.tokens)
}

fn compute_confidence(
    path: &Path,
    size: u64,
    best_match_score: f32,
    name: &str,
    apps: &[InstalledApp],
) -> f32 {
    let mut score = base_confidence(path, size, best_match_score);

    // Signal 4 (Windows) — install location missing from disk (+0.15)
    let uninstalled_boost = apps.iter().any(|app| {
        let folder_lower = name.to_lowercase();
        let app_lower = app.name.to_lowercase();
        (folder_lower.contains(&app_lower) || app_lower.contains(&folder_lower))
            && app
                .install_location
                .as_ref()
                .is_some_and(|loc| !loc.exists())
    });
    if uninstalled_boost {
        score += 0.15;
    }

    score.clamp(0.0, 1.0)
}

fn score_candidate(
    path: PathBuf,
    name: &str,
    apps: &[InstalledApp],
    orphans: &mut Vec<OrphanCandidate>,
) {
    if EXCLUDED_NAMES.contains(&name) {
        return;
    }

    let name_lower = name.to_lowercase();
    if SYSTEM_NAME_PREFIXES
        .iter()
        .any(|p| name_lower.starts_with(p))
    {
        return;
    }

    let tokens = tokenize_name(name);
    let max_score = apps
        .iter()
        .map(|app| correlate(name, &tokens, app))
        .fold(0.0f32, f32::max);

    if max_score >= 0.6 {
        return;
    }

    let size = super::entry_size(&path);
    let confidence = compute_confidence(&path, size, max_score, name, apps);

    orphans.push(OrphanCandidate {
        size,
        confidence,
        path,
    });
}

fn find_orphans(apps: &[InstalledApp], spinner: &ProgressBar) -> Vec<OrphanCandidate> {
    let mut orphans = Vec::new();

    let scan_dirs: Vec<(&str, Option<PathBuf>)> = vec![
        ("AppData\\Roaming", dirs::config_dir()),
        ("AppData\\Local", dirs::data_local_dir()),
    ];

    for (label, dir_opt) in &scan_dirs {
        let Some(dir) = dir_opt else { continue };
        if !dir.exists() {
            continue;
        }
        spinner.set_message(format!("Scanning {label}..."));

        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            score_candidate(path, &name, apps, &mut orphans);
        }
    }

    orphans.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_display_name() {
        let tokens = tokenize_name("Visual Studio Code");
        assert_eq!(tokens, vec!["visual", "studio", "code"]);
    }

    #[test]
    fn tokenize_publisher_with_separators() {
        let tokens = tokenize_name("JetBrains-s.r.o.");
        assert!(tokens.contains(&"jetbrains".to_string()));
        assert!(tokens.contains(&"s".to_string()));
        assert!(tokens.contains(&"r".to_string()));
    }

    #[test]
    fn tokenize_filters_noise() {
        let tokens = tokenize_name("com.org.net.app.io.the.get.real");
        assert_eq!(tokens, vec!["real"]);
    }

    #[test]
    fn tokenize_empty_string() {
        let tokens = tokenize_name("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn build_tokens_deduplicates() {
        let tokens = build_tokens("Spotify", "Spotify AB");
        let spotify_count = tokens.iter().filter(|t| *t == "spotify").count();
        assert_eq!(spotify_count, 1);
    }

    #[test]
    fn build_tokens_empty_publisher() {
        let tokens = build_tokens("Firefox", "");
        assert!(tokens.contains(&"firefox".to_string()));
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn correlate_exact_name_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            install_location: None,
            tokens: build_tokens("Spotify", "Spotify AB"),
        };
        let tokens = tokenize_name("Spotify");
        let score = correlate("Spotify", &tokens, &app);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn correlate_case_insensitive_name_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            install_location: None,
            tokens: build_tokens("Spotify", "Spotify AB"),
        };
        let tokens = tokenize_name("spotify");
        let score = correlate("spotify", &tokens, &app);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn correlate_partial_token_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            install_location: None,
            tokens: build_tokens("Spotify", "Spotify AB"),
        };
        let tokens = tokenize_name("spotify-cache");
        let score = correlate("spotify-cache", &tokens, &app);
        assert!(score > 0.0);
        assert!(score < 1.0);
    }

    #[test]
    fn correlate_no_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            install_location: None,
            tokens: build_tokens("Spotify", "Spotify AB"),
        };
        let tokens = tokenize_name("firefox-data");
        let score = correlate("firefox-data", &tokens, &app);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn excluded_names_are_filtered() {
        let apps: Vec<InstalledApp> = vec![];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        for name in ["Microsoft", "Windows", "pip", "npm", "CrashDumps", "Temp"] {
            let path = tmp.path().join(name);
            std::fs::create_dir(&path).unwrap();
            score_candidate(path, name, &apps, &mut orphans);
        }

        assert!(
            orphans.is_empty(),
            "excluded names should be filtered, got {} orphans",
            orphans.len()
        );
    }

    #[test]
    fn system_prefixes_are_filtered() {
        let apps: Vec<InstalledApp> = vec![];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "Microsoft.NET",
            "microsoft.windowscommunicationsapps",
            "Windows Update",
            "{12345-ABCDE}",
        ] {
            let path = tmp.path().join(name);
            std::fs::create_dir(&path).unwrap();
            score_candidate(path, name, &apps, &mut orphans);
        }

        assert!(
            orphans.is_empty(),
            "system-prefixed names should be filtered, got {} orphans",
            orphans.len()
        );
    }

    #[test]
    fn non_excluded_name_becomes_orphan() {
        let apps: Vec<InstalledApp> = vec![];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("SomeRandomApp");
        std::fs::create_dir(&path).unwrap();

        score_candidate(path, "SomeRandomApp", &apps, &mut orphans);

        assert_eq!(orphans.len(), 1);
    }

    #[test]
    fn strong_match_is_filtered_out() {
        let apps = vec![InstalledApp {
            name: "Spotify".to_string(),
            install_location: None,
            tokens: build_tokens("Spotify", "Spotify AB"),
        }];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Spotify");
        std::fs::create_dir(&path).unwrap();

        score_candidate(path, "Spotify", &apps, &mut orphans);

        assert!(orphans.is_empty(), "exact app match should be filtered out");
    }

    #[test]
    fn confidence_low_for_recent_items() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("recent-thing");
        std::fs::create_dir(&path).unwrap();

        let apps: Vec<InstalledApp> = vec![];
        let score = compute_confidence(&path, 50_000, 0.0, "recent-thing", &apps);
        assert!(
            score < 0.2,
            "recently created item should have low confidence, got {score}"
        );
    }

    #[test]
    fn confidence_clamped_between_0_and_1() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test");
        std::fs::write(&path, "data").unwrap();

        let apps: Vec<InstalledApp> = vec![];
        for best_match in [0.0, 0.1, 0.29, 0.5, 0.59] {
            for size in [0u64, 500, 50_000, 200_000_000] {
                for name in ["a", "some.app.thing", "ALL CAPS", "normal-name"] {
                    let c = compute_confidence(&path, size, best_match, name, &apps);
                    assert!(
                        (0.0..=1.0).contains(&c),
                        "confidence {c} out of range for match={best_match}, size={size}, name={name}"
                    );
                }
            }
        }
    }

    #[test]
    fn uninstalled_app_boosts_confidence() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("OldApp");
        std::fs::create_dir(&path).unwrap();

        let apps_without_loc: Vec<InstalledApp> = vec![];
        let score_no_boost = compute_confidence(&path, 50_000, 0.0, "OldApp", &apps_without_loc);

        // App with install_location pointing to non-existent path
        let apps_with_loc = vec![InstalledApp {
            name: "OldApp".to_string(),
            install_location: Some(PathBuf::from("C:\\NonExistent\\Path\\OldApp")),
            tokens: build_tokens("OldApp", ""),
        }];
        let score_with_boost = compute_confidence(&path, 50_000, 0.0, "OldApp", &apps_with_loc);

        assert!(
            score_with_boost > score_no_boost,
            "uninstalled app should boost confidence: {score_with_boost} > {score_no_boost}"
        );
    }
}
