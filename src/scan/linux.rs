use std::path::{Path, PathBuf};
use std::process::Command;

use indicatif::{ProgressBar, ProgressStyle};

use super::OrphanCandidate;
use super::{base_confidence, build_tokens, token_overlap, tokenize_name};

const EXCLUDED_NAMES: &[&str] = &[
    // Desktop environments
    "gnome",
    "kde",
    "plasma",
    "xfce",
    "cinnamon",
    "mate",
    "sway",
    "hyprland",
    "i3",
    // System
    "dconf",
    "systemd",
    "dbus",
    "fontconfig",
    "ibus",
    "fcitx",
    "xorg",
    "wayland",
    "pipewire",
    "pulseaudio",
    "gvfs",
    "Trash",
    "recently-used.xbel",
    "user-dirs.dirs",
    "user-dirs.locale",
    // Dev tools
    "pip",
    "npm",
    "yarn",
    "cargo",
    "rustup",
    "gem",
    "go",
    "pnpm",
    "uv",
    "node-gyp",
    // Package manager data
    "flatpak",
    "snap",
    "dpkg",
    "apt",
    "pacman",
    "yay",
    "pamac",
];

/// Prefixes that indicate system/DE entries — filter entirely
const SYSTEM_PREFIXES: &[&str] = &["org.gnome.", "org.kde.", "org.freedesktop."];

struct InstalledApp {
    name: String,
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
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 1. Parse .desktop files
    let desktop_dirs = desktop_file_dirs();
    for dir in &desktop_dirs {
        if !dir.exists() {
            continue;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "desktop")
                && let Some(app) = parse_desktop_file(&path)
            {
                let key = app.name.to_lowercase();
                if seen.insert(key) {
                    spinner.set_message(format!("Found: {}", app.name));
                    apps.push(app);
                }
            }
        }
    }

    // 2. Package managers (best-effort)
    if let Some(pkg_apps) = query_dpkg() {
        for name in pkg_apps {
            let key = name.to_lowercase();
            if seen.insert(key) {
                let tokens = build_tokens(&name, "");
                apps.push(InstalledApp { name, tokens });
            }
        }
    } else if let Some(pkg_apps) = query_rpm() {
        for name in pkg_apps {
            let key = name.to_lowercase();
            if seen.insert(key) {
                let tokens = build_tokens(&name, "");
                apps.push(InstalledApp { name, tokens });
            }
        }
    } else if let Some(pkg_apps) = query_pacman() {
        for name in pkg_apps {
            let key = name.to_lowercase();
            if seen.insert(key) {
                let tokens = build_tokens(&name, "");
                apps.push(InstalledApp { name, tokens });
            }
        }
    }

    // 3. Flatpak
    if let Some(flatpak_apps) = query_flatpak() {
        for (app_id, display_name) in flatpak_apps {
            let extra = &app_id;
            let key = display_name.to_lowercase();
            if seen.insert(key) {
                let tokens = build_tokens(&display_name, extra);
                spinner.set_message(format!("Found (flatpak): {display_name}"));
                apps.push(InstalledApp {
                    name: display_name,
                    tokens,
                });
            }
        }
    }

    // 4. Snap
    if let Some(snap_apps) = discover_snaps() {
        for name in snap_apps {
            let key = name.to_lowercase();
            if seen.insert(key) {
                let tokens = build_tokens(&name, "");
                spinner.set_message(format!("Found (snap): {name}"));
                apps.push(InstalledApp { name, tokens });
            }
        }
    }

    apps
}

fn desktop_file_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    dirs
}

fn parse_desktop_file(path: &Path) -> Option<InstalledApp> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut exec = None;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_desktop_entry = trimmed == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry {
            continue;
        }
        if let Some(val) = trimmed.strip_prefix("Name=") {
            if name.is_none() {
                name = Some(val.to_string());
            }
        } else if let Some(val) = trimmed.strip_prefix("Exec=")
            && exec.is_none()
        {
            // Extract binary name: first token, strip path
            let binary = val
                .split_whitespace()
                .next()
                .unwrap_or("")
                .rsplit('/')
                .next()
                .unwrap_or("");
            if !binary.is_empty() {
                exec = Some(binary.to_string());
            }
        }
    }

    let app_name = name?;
    let extra = exec.as_deref().unwrap_or("");
    let tokens = build_tokens(&app_name, extra);

    Some(InstalledApp {
        name: app_name,
        tokens,
    })
}

fn query_dpkg() -> Option<Vec<String>> {
    let output = Command::new("dpkg")
        .args(["--get-selections"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let status = parts.next().unwrap_or("");
            if status == "deinstall" {
                return None;
            }
            // Strip architecture suffix (e.g., "libfoo:amd64")
            Some(name.split(':').next().unwrap_or(name).to_string())
        })
        .collect();
    Some(names)
}

fn query_rpm() -> Option<Vec<String>> {
    let output = Command::new("rpm")
        .args(["-qa", "--qf", "%{NAME}\n"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().map(|l| l.to_string()).collect())
}

fn query_pacman() -> Option<Vec<String>> {
    let output = Command::new("pacman").args(["-Qq"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().map(|l| l.to_string()).collect())
}

fn query_flatpak() -> Option<Vec<(String, String)>> {
    let output = Command::new("flatpak")
        .args(["list", "--app", "--columns=application,name"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let apps: Vec<(String, String)> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let app_id = parts.next()?.trim().to_string();
            let name = parts
                .next()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if app_id.is_empty() {
                return None;
            }
            let display = if name.is_empty() {
                app_id.clone()
            } else {
                name
            };
            Some((app_id, display))
        })
        .collect();
    Some(apps)
}

fn discover_snaps() -> Option<Vec<String>> {
    let snap_dir = PathBuf::from("/snap");
    if !snap_dir.is_dir() {
        return None;
    }
    let entries = std::fs::read_dir(&snap_dir).ok()?;
    let names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            // Skip snap internal dirs
            if name == "bin" || name == "snapd" || name == "core" || name.starts_with("core") {
                return None;
            }
            Some(name)
        })
        .collect();
    Some(names)
}

fn correlate(folder_name: &str, folder_tokens: &[String], app: &InstalledApp) -> f32 {
    // Exact name match (case-insensitive)
    if folder_name.eq_ignore_ascii_case(&app.name) {
        return 1.0;
    }
    token_overlap(folder_tokens, &app.tokens)
}

fn compute_confidence(path: &Path, size: u64, best_match_score: f32, name: &str) -> f32 {
    let mut score = base_confidence(path, size, best_match_score);

    // Signal 4 (Linux) — dotfile/hidden directory penalty (-0.1)
    if name.starts_with('.') {
        score -= 0.1;
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
    if SYSTEM_PREFIXES.iter().any(|p| name_lower.starts_with(p)) {
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

fn scan_dirs() -> Vec<(&'static str, PathBuf)> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let config = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".config"));
    let data = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".local/share"));
    let cache = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".cache"));

    vec![
        ("~/.config", config),
        ("~/.local/share", data),
        ("~/.cache", cache),
    ]
}

fn find_orphans(home: &Path, apps: &[InstalledApp], spinner: &ProgressBar) -> Vec<OrphanCandidate> {
    let _ = home; // home already used via scan_dirs()
    let mut orphans = Vec::new();

    for (label, dir) in scan_dirs() {
        if !dir.exists() {
            continue;
        }
        spinner.set_message(format!("Scanning {label}..."));

        let entries = match std::fs::read_dir(&dir) {
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
    fn parse_desktop_file_extracts_name_and_exec() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop = tmp.path().join("test.desktop");
        std::fs::write(
            &desktop,
            "[Desktop Entry]\nName=My App\nExec=/usr/bin/myapp --flag\nType=Application\n",
        )
        .unwrap();

        let app = parse_desktop_file(&desktop).unwrap();
        assert_eq!(app.name, "My App");
        assert!(app.tokens.contains(&"my".to_string()));
        assert!(app.tokens.contains(&"myapp".to_string()));
    }

    #[test]
    fn parse_desktop_file_ignores_non_desktop_entry_sections() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop = tmp.path().join("test.desktop");
        std::fs::write(
            &desktop,
            "[Desktop Action New]\nName=New Window\nExec=foo --new\n\n[Desktop Entry]\nName=Real App\nExec=realapp\n",
        )
        .unwrap();

        let app = parse_desktop_file(&desktop).unwrap();
        assert_eq!(app.name, "Real App");
        assert!(app.tokens.contains(&"realapp".to_string()));
    }

    #[test]
    fn parse_desktop_file_missing_name_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        let desktop = tmp.path().join("test.desktop");
        std::fs::write(&desktop, "[Desktop Entry]\nExec=foo\n").unwrap();

        assert!(parse_desktop_file(&desktop).is_none());
    }

    #[test]
    fn excluded_names_are_filtered() {
        let apps: Vec<InstalledApp> = vec![];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        for name in ["gnome", "systemd", "pip", "npm", "flatpak", "dconf"] {
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
            "org.gnome.Weather",
            "org.kde.dolphin",
            "org.freedesktop.secrets",
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
            name: "Firefox".to_string(),
            tokens: build_tokens("Firefox", "firefox"),
        }];
        let mut orphans = Vec::new();

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("Firefox");
        std::fs::create_dir(&path).unwrap();

        score_candidate(path, "Firefox", &apps, &mut orphans);

        assert!(orphans.is_empty(), "exact app match should be filtered out");
    }

    #[test]
    fn hidden_dir_gets_confidence_penalty() {
        let tmp = tempfile::tempdir().unwrap();
        let hidden = tmp.path().join(".hidden-app");
        std::fs::create_dir(&hidden).unwrap();
        let visible = tmp.path().join("visible-app");
        std::fs::create_dir(&visible).unwrap();

        let score_hidden = compute_confidence(&hidden, 1000, 0.0, ".hidden-app");
        let score_visible = compute_confidence(&visible, 1000, 0.0, "visible-app");

        assert!(
            score_hidden < score_visible,
            "hidden dir ({score_hidden}) should have lower confidence than visible ({score_visible})"
        );
    }

    #[test]
    fn confidence_clamped_between_0_and_1() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test");
        std::fs::write(&path, "data").unwrap();

        for best_match in [0.0, 0.1, 0.29, 0.5, 0.59] {
            for size in [0u64, 500, 50_000, 200_000_000] {
                for name in ["a", ".hidden", "normal-name", "some.app"] {
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
    fn correlate_exact_name_match() {
        let app = InstalledApp {
            name: "Firefox".to_string(),
            tokens: build_tokens("Firefox", "firefox"),
        };
        let tokens = tokenize_name("Firefox");
        let score = correlate("Firefox", &tokens, &app);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn correlate_case_insensitive() {
        let app = InstalledApp {
            name: "Firefox".to_string(),
            tokens: build_tokens("Firefox", "firefox"),
        };
        let tokens = tokenize_name("firefox");
        let score = correlate("firefox", &tokens, &app);
        assert_eq!(score, 1.0);
    }

    #[test]
    fn correlate_no_match() {
        let app = InstalledApp {
            name: "Firefox".to_string(),
            tokens: build_tokens("Firefox", "firefox"),
        };
        let tokens = tokenize_name("spotify-data");
        let score = correlate("spotify-data", &tokens, &app);
        assert_eq!(score, 0.0);
    }
}
