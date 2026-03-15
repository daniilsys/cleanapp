# cleanapp

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)](https://www.apple.com/macos/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-blue.svg)](Cargo.toml)

A macOS CLI tool to find and remove leftover files and directories from uninstalled applications.

## Usage

```bash
cleanapp <APP_NAME> [OPTIONS]
```

## Options

| Flag               | Description                                                          |
| ------------------ | -------------------------------------------------------------------- |
| `--exclude <NAME>` | Exclude paths containing this term (repeatable)                      |
| `--deep`           | Deep search across the entire home directory (slower but thorough)   |
| `--case-sensitive` | Match the app name with exact casing (default is case-insensitive)   |
| `--exact`          | Match the app name as a whole word, not as a substring               |

## Examples

```zsh
# Delete all Spotify leftover files
cleanapp Spotify

# Preview Chrome files, excluding Arc-related paths
cleanapp Chrome --exclude Arc

# Clean system-level files (requires sudo)
sudo cleanapp Zoom

# Deep search for all Firefox remnants
cleanapp Firefox --deep

# Find "chrome" but not "chromium"
cleanapp chrome --exact

# Case-sensitive deep search
cleanapp WebStorm --case-sensitive --deep
```

## Install

```zsh
git clone https://github.com/daniilsys/cleanapp
cd cleanapp
cargo install --path .
```

## Notes

- Searches `~/Library` and `/Library` by default, `--deep` scans the entire home directory
- Matching is case-insensitive by default
- When a matching directory is found, its contents are not scanned individually
- Deleting from `/Library` may require `sudo`

## License

MIT. SEE: `LICENSE`
