# cleanapp

[![CI](https://github.com/daniilsys/cleanapp/actions/workflows/ci.yml/badge.svg)](https://github.com/daniilsys/cleanapp/actions/workflows/ci.yml)
[![Release](https://github.com/daniilsys/cleanapp/actions/workflows/release.yml/badge.svg)](https://github.com/daniilsys/cleanapp/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.3.1-blue.svg)](Cargo.toml)

[![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)](https://www.apple.com/macos/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black)](https://www.linux.org/)
[![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)

A cross-platform CLI tool to find and remove leftover files and directories from uninstalled applications. Supports targeted cleanup by app name and automatic orphan detection on macOS, Linux and Windows.

## Install

### Download binary (no Rust required)

Download the latest binary for your platform from [GitHub Releases](https://github.com/daniilsys/cleanapp/releases/latest).

**macOS / Linux:**

```bash
# macOS ARM (Apple Silicon)
curl -fsSL https://github.com/daniilsys/cleanapp/releases/latest/download/cleanapp-macos-arm64 -o cleanapp

# macOS Intel
curl -fsSL https://github.com/daniilsys/cleanapp/releases/latest/download/cleanapp-macos-x86_64 -o cleanapp

# Linux
curl -fsSL https://github.com/daniilsys/cleanapp/releases/latest/download/cleanapp-linux-x86_64 -o cleanapp

chmod +x cleanapp
sudo mv cleanapp /usr/local/bin/
```

**Windows (PowerShell):**

```powershell
Invoke-WebRequest -Uri https://github.com/daniilsys/cleanapp/releases/latest/download/cleanapp-windows-x86_64.exe -OutFile cleanapp.exe
Move-Item cleanapp.exe C:\Windows\System32\
```

### Build from source

```bash
# From GitHub directly
cargo install --git https://github.com/daniilsys/cleanapp

# Or clone and build
git clone https://github.com/daniilsys/cleanapp
cd cleanapp
cargo install --path .
```

## Usage

cleanapp has two subcommands: `clean` for targeted removal by app name, and `scan` for automatic orphan detection.

### `cleanapp clean` — Remove files by app name

```bash
cleanapp clean <APP_NAME> [OPTIONS]
```

| Flag               | Description                                                        |
| ------------------ | ------------------------------------------------------------------ |
| `--exclude <NAME>` | Exclude paths containing this term (repeatable)                    |
| `--deep`           | Deep search across the entire home directory (slower but thorough) |
| `--case-sensitive` | Match the app name with exact casing (default is case-insensitive) |
| `--exact`          | Match the app name as a whole word, not as a substring             |
| `--here`           | Search only in the current directory and its subdirectories        |
| `--max-depth <N>`  | Maximum depth of subdirectories to search                          |
| `--add <PATH>`     | Add a custom path to search in (repeatable)                        |

### `cleanapp scan` — Detect orphan files (macOS, Linux & Windows)

```bash
cleanapp scan [OPTIONS]
```

Scans installed applications and checks support directories for entries that don't match any installed app. Each orphan candidate receives a confidence score (0-100%) based on match quality, file age, size, and name format.

- **macOS**: discovers apps via `/Applications` bundle IDs, scans `~/Library` support directories
- **Linux**: discovers apps via `.desktop` files, package managers (dpkg/rpm/pacman), Flatpak and Snap; scans `~/.config`, `~/.local/share`, `~/.cache`
- **Windows**: discovers apps via the registry and Scoop, scans `%APPDATA%` and `%LOCALAPPDATA%`

| Flag                   | Description                                                           |
| ---------------------- | --------------------------------------------------------------------- |
| `--confidence <VALUE>` | Pre-select orphans with confidence >= this threshold (default: `0.5`) |
| `--atleast <VALUE>`    | Only show orphans with confidence >= this value (hides the rest)      |

## Examples

```bash
# Delete all Spotify leftover files
cleanapp clean Spotify

# Preview Chrome files, excluding Arc-related paths
cleanapp clean Chrome --exclude Arc

# Clean system-level files (requires elevated privileges)
# macOS/Linux:
sudo cleanapp clean Zoom
# Windows (run terminal as Administrator):
cleanapp clean Zoom

# Deep search for all Firefox remnants
cleanapp clean Firefox --deep

# Find "chrome" but not "chromium"
cleanapp clean chrome --exact

# Case-sensitive deep search
cleanapp clean WebStorm --case-sensitive --deep

# Search only in the current directory
cleanapp clean node_modules --here

# Limit search depth to 3 levels
cleanapp clean Spotify --max-depth 3

# Add custom paths to search in
cleanapp clean Zoom --add /tmp --add /var/log

# Combine options
cleanapp clean Chrome --here --add ~/Downloads --max-depth 2

# Scan for orphan files (macOS/Linux/Windows)
cleanapp scan

# Only pre-select high-confidence orphans
cleanapp scan --confidence 0.8

# Hide low-confidence results entirely
cleanapp scan --atleast 0.7
```

## Supported platforms

| Platform | Default search paths                      |
| -------- | ----------------------------------------- |
| macOS    | `~/Library`, `/Library`                   |
| Linux    | `~/.cache`, `~/.config`, `~/.local/share` |
| Windows  | `%APPDATA%`, `%LOCALAPPDATA%`             |

Use `--deep` to scan the entire home directory on any platform.

## Notes

- `clean` matching is case-insensitive by default
- When a matching directory is found, its contents are not scanned individually
- Deleting system-level files may require elevated privileges (sudo on macOS/Linux, Administrator on Windows)
- `scan` is supported on macOS, Linux and Windows

## License

MIT. See [LICENSE](LICENSE).

Made by daniilsys with love ❤️
