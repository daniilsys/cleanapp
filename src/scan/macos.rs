use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

use super::OrphanCandidate;
use super::{base_confidence, build_tokens, token_overlap, tokenize_name};

const APP_ROOTS: &[&str] = &[
    "/Applications",
    "/System/Applications",
    "/System/Applications/Utilities",
    "/System/Library/CoreServices",
];

const SUPPORT_DIRS: &[&str] = &[
    "Library/Application Support",
    "Library/Caches",
    "Library/Logs",
];

const EXCLUDED_NAMES: &[&str] = &[
    // CLI tooling — not apps
    "pip",
    "pnpm",
    "npm",
    "node-gyp",
    "typescript",
    "yarn",
    "Homebrew",
    "cargo",
    "rustup",
    "gem",
    "uv",
    "prisma-nodejs",
    "prisma-dev-nodejs",
    // macOS daemons and system services
    "networkserviceproxy",
    "identityservicesd",
    "locationaccessstored",
    "appplaceholdersyncd",
    "tipsd",
    "contactsd",
    "icloudmailagent",
    "homeenergyd",
    "mbuseragent",
    "askpermissiond",
    "ssu",
    "familycircled",
    "AMSDataMigratorTool",
    // Legitimate system caches
    "GeoServices",
    "CloudKit",
    "GameKit",
    "PassKit",
    "SiriTTS",
    "SiriTTSService",
    "PrivacyPreservationMeasurement",
    // Sync and iOS/iCloud data
    "SyncServices",
    "MobileSync",
    "CallHistoryDB",
    "CallHistoryTransactions",
    "CloudDocs",
    "iCloud",
    "iLifeMediaBrowser",
    // Apple frameworks and daemons
    "Animoji",
    "DifferentialPrivacy",
    "ConfigurationProfiles",
    "DiskImages",
    "icdd",
    "Knowledge",
    // System logs and crash data
    "CrashReporter",
    "DiagnosticReports",
    "Baseband",
    // System preferences without com.apple prefix
    "loginwindow",
    "pbs",
    "sharedfilelistd",
    "MobileMeAccounts",
    "ContextStoreAgent",
    ".GlobalPreferences",
    ".GlobalPreferences_m",
    // CLI tools without .app bundles
    "python",
    "mpv",
    "MiniLauncher",
    // Third-party SDKs and libraries (embedded in apps, not standalone)
    "SentryCrash",
    "segment",
    "JNA",
    "SESStorage",
    // Generic directory names
    "Caches",
    // Root-level files unrelated to apps
    "INSTALLATION",
];

const PREFERENCES_DIR: &str = "Library/Preferences";

/// Prefixes that indicate system-level plists in Preferences
const SYSTEM_PLIST_PREFIXES: &[&str] = &["com.apple.", "apple.", "org.cups."];

struct InstalledApp {
    name: String,
    bundle_id: String,
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

    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let orphans = find_orphans(&home, &apps, &spinner);
    spinner.finish_and_clear();

    super::present_orphans(orphans, min_confidence, min_threshold)
}

fn discover_installed_apps(spinner: &ProgressBar) -> Vec<InstalledApp> {
    let mut apps = Vec::new();

    for root in APP_ROOTS {
        for entry in WalkDir::new(root)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "app") {
                let plist_path = path.join("Contents/Info.plist");
                if plist_path.exists()
                    && let Some(app) = parse_info_plist(&plist_path)
                {
                    spinner.set_message(format!("Found: {}", app.name));
                    apps.push(app);
                }
            }
        }
    }

    apps
}

fn parse_info_plist(path: &Path) -> Option<InstalledApp> {
    let value: plist::Dictionary = plist::from_file(path).ok()?;

    let bundle_id = value.get("CFBundleIdentifier")?.as_string()?.to_string();
    let name = value
        .get("CFBundleName")
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| {
            // Fallback: derive name from bundle_id last component
            bundle_id.rsplit('.').next().unwrap_or(&bundle_id)
        })
        .to_string();

    let tokens = build_tokens(&name, &bundle_id);

    Some(InstalledApp {
        name,
        bundle_id,
        tokens,
    })
}

fn correlate(folder_name: &str, folder_tokens: &[String], app: &InstalledApp) -> f32 {
    if folder_name == app.bundle_id {
        return 1.0;
    }
    token_overlap(folder_tokens, &app.tokens)
}

fn compute_confidence(path: &Path, size: u64, best_match_score: f32, name: &str) -> f32 {
    let mut score = base_confidence(path, size, best_match_score);

    // Signal 4 (macOS) — name format (-0.1 to +0.1)
    let dot_count = name.chars().filter(|&c| c == '.').count();
    if dot_count >= 2 {
        score += 0.1; // bundle ID format (com.xxx.yyy)
    } else if !name.is_empty() && name.chars().all(|c| c.is_uppercase() || c == ' ') {
        score -= 0.1; // all uppercase = often system
    }

    score.clamp(0.0, 1.0)
}

/// Score a candidate entry against installed apps and push to orphans if it qualifies.
fn score_candidate(
    path: PathBuf,
    name: &str,
    apps: &[InstalledApp],
    orphans: &mut Vec<OrphanCandidate>,
) {
    if EXCLUDED_NAMES.contains(&name) {
        return;
    }
    if SYSTEM_PLIST_PREFIXES.iter().any(|p| name.starts_with(p)) {
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
    let confidence = compute_confidence(&path, size, max_score, name);

    orphans.push(OrphanCandidate {
        size,
        confidence,
        path,
    });
}

fn find_orphans(home: &Path, apps: &[InstalledApp], spinner: &ProgressBar) -> Vec<OrphanCandidate> {
    let mut orphans = Vec::new();

    // Scan regular support dirs (top-level entries)
    for dir_name in SUPPORT_DIRS {
        let dir = home.join(dir_name);
        if !dir.exists() {
            continue;
        }
        spinner.set_message(format!("Scanning {dir_name}..."));

        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Ignore root-level files that aren't dirs or .plist
            if path.is_file() && path.extension().is_none_or(|e| e != "plist") {
                continue;
            }

            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            score_candidate(path, &name, apps, &mut orphans);
        }
    }

    // Scan Preferences (.plist files only)
    let prefs = home.join(PREFERENCES_DIR);
    if prefs.exists() {
        spinner.set_message(format!("Scanning {PREFERENCES_DIR}..."));
        if let Ok(entries) = std::fs::read_dir(&prefs) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "plist") {
                    let name = match path.file_stem().and_then(|n| n.to_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };

                    if name.chars().all(|c| c.is_ascii_digit()) {
                        continue;
                    }

                    score_candidate(path, &name, apps, &mut orphans);
                }
            }
        }
    }

    orphans.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    orphans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_bundle_id() {
        let tokens = tokenize_name("com.spotify.client");
        // "com" is noise, should be filtered
        assert!(!tokens.contains(&"com".to_string()));
        assert!(tokens.contains(&"spotify".to_string()));
        assert!(tokens.contains(&"client".to_string()));
    }

    #[test]
    fn tokenize_app_name() {
        let tokens = tokenize_name("Visual Studio Code");
        assert_eq!(tokens, vec!["visual", "studio", "code"]);
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
        let tokens = build_tokens("Spotify", "com.spotify.client");
        let spotify_count = tokens.iter().filter(|t| *t == "spotify").count();
        assert_eq!(spotify_count, 1);
    }

    #[test]
    fn correlate_exact_bundle_id_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            bundle_id: "com.spotify.client".to_string(),
            tokens: build_tokens("Spotify", "com.spotify.client"),
        };
        let score = correlate(
            "com.spotify.client",
            &tokenize_name("com.spotify.client"),
            &app,
        );
        assert_eq!(score, 1.0);
    }

    #[test]
    fn correlate_partial_token_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            bundle_id: "com.spotify.client".to_string(),
            tokens: build_tokens("Spotify", "com.spotify.client"),
        };
        // "spotify" matches, "cache" does not
        let tokens = tokenize_name("spotify-cache");
        let score = correlate("spotify-cache", &tokens, &app);
        assert!(score > 0.0);
        assert!(score < 1.0);
        // 1 match out of 2 tokens = 0.5
        assert_eq!(score, 0.5);
    }

    #[test]
    fn correlate_no_match() {
        let app = InstalledApp {
            name: "Spotify".to_string(),
            bundle_id: "com.spotify.client".to_string(),
            tokens: build_tokens("Spotify", "com.spotify.client"),
        };
        let tokens = tokenize_name("firefox-data");
        let score = correlate("firefox-data", &tokens, &app);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn correlate_empty_tokens() {
        let app = InstalledApp {
            name: "Test".to_string(),
            bundle_id: "com.test".to_string(),
            tokens: build_tokens("Test", "com.test"),
        };
        // All noise tokens → empty after filtering
        let tokens = tokenize_name("com.org.net");
        let score = correlate("com.org.net", &tokens, &app);
        // 0 / max(0, 1) = 0.0
        assert_eq!(score, 0.0);
    }

    #[test]
    fn excluded_dirs_are_filtered_by_find_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Create a support dir with excluded and non-excluded entries
        let app_support = home.join("Library/Application Support");
        std::fs::create_dir_all(&app_support).unwrap();

        for name in ["Homebrew", "pip", "npm", "GeoServices", "CloudKit"] {
            std::fs::create_dir(app_support.join(name)).unwrap();
        }
        // A non-excluded dir that won't match any app → should be an orphan
        std::fs::create_dir(app_support.join("SomeRandomApp")).unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps: Vec<InstalledApp> = vec![];
        let orphans = find_orphans(home, &apps, &pb);

        let orphan_names: Vec<String> = orphans
            .iter()
            .filter_map(|o| {
                o.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        // Excluded dirs must not appear
        for excluded in ["Homebrew", "pip", "npm", "GeoServices", "CloudKit"] {
            assert!(
                !orphan_names.contains(&excluded.to_string()),
                "{excluded} should be excluded but was found in orphans"
            );
        }

        // Non-excluded dir should appear
        assert!(
            orphan_names.contains(&"SomeRandomApp".to_string()),
            "SomeRandomApp should be detected as orphan"
        );
    }

    #[test]
    fn system_plist_prefixes_are_filtered_in_preferences() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        let prefs = home.join("Library/Preferences");
        std::fs::create_dir_all(&prefs).unwrap();

        // System plists — should be filtered
        std::fs::write(prefs.join("com.apple.finder.plist"), "data").unwrap();
        std::fs::write(prefs.join("com.apple.dock.plist"), "data").unwrap();
        std::fs::write(prefs.join("apple.some-daemon.plist"), "data").unwrap();
        std::fs::write(prefs.join("org.cups.PrintingPrefs.plist"), "data").unwrap();

        // Non-system plist — should appear as orphan
        std::fs::write(prefs.join("com.example.myapp.plist"), "data").unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps: Vec<InstalledApp> = vec![];
        let orphans = find_orphans(home, &apps, &pb);

        let orphan_stems: Vec<String> = orphans
            .iter()
            .filter_map(|o| {
                o.path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        for filtered in [
            "com.apple.finder",
            "com.apple.dock",
            "apple.some-daemon",
            "org.cups.PrintingPrefs",
        ] {
            assert!(
                !orphan_stems.contains(&filtered.to_string()),
                "{filtered} should be filtered"
            );
        }
        assert!(
            orphan_stems.contains(&"com.example.myapp".to_string()),
            "com.example.myapp should appear as orphan"
        );
    }

    #[test]
    fn numeric_plist_stems_are_filtered_in_preferences() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        let prefs = home.join("Library/Preferences");
        std::fs::create_dir_all(&prefs).unwrap();

        // Purely numeric stems — should be filtered
        std::fs::write(prefs.join("20631581080.plist"), "data").unwrap();
        std::fs::write(prefs.join("12345.plist"), "data").unwrap();

        // Non-numeric — should appear as orphan
        std::fs::write(prefs.join("com.example.tool.plist"), "data").unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps: Vec<InstalledApp> = vec![];
        let orphans = find_orphans(home, &apps, &pb);

        let orphan_stems: Vec<String> = orphans
            .iter()
            .filter_map(|o| {
                o.path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        assert!(
            !orphan_stems.contains(&"20631581080".to_string()),
            "numeric stem should be filtered"
        );
        assert!(
            !orphan_stems.contains(&"12345".to_string()),
            "numeric stem should be filtered"
        );
        assert!(
            orphan_stems.contains(&"com.example.tool".to_string()),
            "non-numeric stem should appear as orphan"
        );
    }

    #[test]
    fn system_prefixes_filtered_in_support_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        let caches = home.join("Library/Caches");
        std::fs::create_dir_all(&caches).unwrap();

        std::fs::create_dir(caches.join("com.apple.something")).unwrap();
        std::fs::create_dir(caches.join("com.example.orphan")).unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps: Vec<InstalledApp> = vec![];
        let orphans = find_orphans(home, &apps, &pb);

        let orphan_names: Vec<String> = orphans
            .iter()
            .filter_map(|o| {
                o.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        assert!(
            !orphan_names.contains(&"com.apple.something".to_string()),
            "com.apple.* should be filtered in support dirs"
        );
        assert!(
            orphan_names.contains(&"com.example.orphan".to_string()),
            "non-system dir should still appear"
        );
    }

    #[test]
    fn confidence_low_for_recent_items() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("recent-thing");
        std::fs::create_dir(&path).unwrap();

        // Recent item (just created), no match, moderate size
        let score = compute_confidence(&path, 50_000, 0.0, "recent-thing");
        // Signal 1: 0.35 + Signal 2: -0.3 (< 7 days) + Signal 3: 0.0 = 0.05
        assert!(
            score < 0.2,
            "recently created item should have low confidence, got {score}"
        );
    }

    #[test]
    fn confidence_empty_dir_scores_higher() {
        let tmp = tempfile::tempdir().unwrap();

        // Empty dir, no match, bundle-id-like name
        let empty_dir = tmp.path().join("com.old.removed");
        std::fs::create_dir(&empty_dir).unwrap();

        // Non-empty dir, no match, simple name
        let nonempty_dir = tmp.path().join("something");
        std::fs::create_dir(&nonempty_dir).unwrap();
        std::fs::write(nonempty_dir.join("data.bin"), vec![0u8; 4096]).unwrap();

        let score_empty = compute_confidence(&empty_dir, 0, 0.0, "com.old.removed");
        let score_nonempty = compute_confidence(&nonempty_dir, 4096, 0.0, "something");

        assert!(
            score_empty > score_nonempty,
            "empty bundle-id dir ({score_empty}) should score higher than non-empty simple dir ({score_nonempty})"
        );
    }

    #[test]
    fn confidence_clamped_between_0_and_1() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test");
        std::fs::write(&path, "data").unwrap();

        // Test with various inputs to ensure clamping
        for best_match in [0.0, 0.1, 0.29, 0.5, 0.59] {
            for size in [0u64, 500, 50_000, 200_000_000] {
                for name in ["a", "com.x.y.z", "ALL CAPS", "normal-name"] {
                    let c = compute_confidence(&path, size, best_match, name);
                    assert!(
                        (0.0..=1.0).contains(&c),
                        "confidence {c} out of range for match={best_match}, size={size}, name={name}"
                    );
                }
            }
        }
    }

    #[test]
    fn find_orphans_sorted_by_confidence_desc() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        let app_support = home.join("Library/Application Support");
        std::fs::create_dir_all(&app_support).unwrap();

        // Create dirs with varying characteristics
        // Empty dir with bundle-id name → high confidence
        std::fs::create_dir(app_support.join("com.removed.app")).unwrap();
        // Dir with some content → lower confidence
        let big_dir = app_support.join("SomeApp");
        std::fs::create_dir(&big_dir).unwrap();
        std::fs::write(big_dir.join("data.bin"), vec![0u8; 2048]).unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps: Vec<InstalledApp> = vec![];
        let orphans = find_orphans(home, &apps, &pb);

        assert!(orphans.len() >= 2, "expected at least 2 orphans");

        // Verify sorted descending
        for window in orphans.windows(2) {
            assert!(
                window[0].confidence >= window[1].confidence,
                "orphans not sorted by confidence desc: {} < {}",
                window[0].confidence,
                window[1].confidence
            );
        }
    }

    #[test]
    fn find_orphans_filters_strong_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        let app_support = home.join("Library/Application Support");
        std::fs::create_dir_all(&app_support).unwrap();

        // This dir name exactly matches the app's bundle_id → score 1.0 → filtered out
        std::fs::create_dir(app_support.join("com.test.myapp")).unwrap();
        // This dir has no match → should be orphan
        std::fs::create_dir(app_support.join("com.unknown.thing")).unwrap();

        let pb = ProgressBar::new_spinner();
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());

        let apps = vec![InstalledApp {
            name: "MyApp".to_string(),
            bundle_id: "com.test.myapp".to_string(),
            tokens: build_tokens("MyApp", "com.test.myapp"),
        }];

        let orphans = find_orphans(home, &apps, &pb);

        let orphan_names: Vec<String> = orphans
            .iter()
            .filter_map(|o| {
                o.path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .collect();

        assert!(
            !orphan_names.contains(&"com.test.myapp".to_string()),
            "strong match should be filtered out"
        );
        assert!(
            orphan_names.contains(&"com.unknown.thing".to_string()),
            "no-match dir should be orphan"
        );
    }
}
