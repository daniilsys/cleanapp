# cleanapp

[![CI](https://github.com/daniilsys/cleanapp/actions/workflows/ci.yml/badge.svg)](https://github.com/daniilsys/cleanapp/actions/workflows/ci.yml)
[![Release](https://github.com/daniilsys/cleanapp/actions/workflows/release.yml/badge.svg)](https://github.com/daniilsys/cleanapp/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.2.0-blue.svg)](Cargo.toml)

[![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)](https://www.apple.com/macos/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=flat&logo=linux&logoColor=black)](https://www.linux.org/)
[![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)](https://www.microsoft.com/windows)
[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)

A cross-platform CLI tool to find and remove leftover files and directories from uninstalled applications.

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

```bash
cleanapp <APP_NAME> [OPTIONS]
```

## Options

| Flag               | Description                                                        |
| ------------------ | ------------------------------------------------------------------ |
| `--exclude <NAME>` | Exclude paths containing this term (repeatable)                    |
| `--deep`           | Deep search across the entire home directory (slower but thorough) |
| `--case-sensitive` | Match the app name with exact casing (default is case-insensitive) |
| `--exact`          | Match the app name as a whole word, not as a substring             |
| `--here`           | Search only in the current directory and its subdirectories        |
| `--max-depth <N>`  | Maximum depth of subdirectories to search                          |
| `--add <PATH>`     | Add a custom path to search in (repeatable)                        |

## Examples

```bash
# Delete all Spotify leftover files
cleanapp Spotify

# Preview Chrome files, excluding Arc-related paths
cleanapp Chrome --exclude Arc

# Clean system-level files (requires elevated privileges)
# macOS/Linux:
sudo cleanapp Zoom
# Windows (run terminal as Administrator):
cleanapp Zoom

# Deep search for all Firefox remnants
cleanapp Firefox --deep

# Find "chrome" but not "chromium"
cleanapp chrome --exact

# Case-sensitive deep search
cleanapp WebStorm --case-sensitive --deep

# Search only in the current directory
cleanapp node_modules --here

# Limit search depth to 3 levels
cleanapp Spotify --max-depth 3

# Add custom paths to search in
cleanapp Zoom --add /tmp --add /var/log

# Combine options
cleanapp Chrome --here --add ~/Downloads --max-depth 2
```

## Supported platforms

| Platform | Default search paths                         |
| -------- | -------------------------------------------- |
| macOS    | `~/Library`, `/Library`                      |
| Linux    | `~/.cache`, `~/.config`, `~/.local/share`    |
| Windows  | `%APPDATA%`, `%LOCALAPPDATA%`                |

Use `--deep` to scan the entire home directory on any platform.

## Notes

- Matching is case-insensitive by default
- When a matching directory is found, its contents are not scanned individually
- Deleting system-level files may require elevated privileges (sudo on macOS/Linux, Administrator on Windows)

## License

MIT. See [LICENSE](LICENSE).

Made by daniilsys with love ❤️
