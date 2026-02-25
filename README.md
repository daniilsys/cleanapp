# cleanapp

A macOS CLI tool to find and remove leftover files and directories from uninstalled applications.

## Usage

```bash
cleanapp <APP_NAME> [OPTIONS]
```

## Options

| Flag               | Description                                     |
| ------------------ | ----------------------------------------------- |
| `--list`           | Preview files and directories before deleting   |
| `--exclude <NAME>` | Exclude paths containing this term (repeatable) |

## Examples

```zsh
# Delete all Spotify leftover files
cleanapp Spotify

# Preview Chrome files, excluding Arc-related paths
cleanapp Chrome --list --exclude Arc

# Clean system-level files (requires sudo)
sudo cleanapp Zoom --list
```

## Install

```zsh
git clone https://github.com/daniilsys/cleanapp
cd cleanapp
cargo install --path .
```

## Notes

- Searches `~/Library` and `/Library`
- Matching is case-insensitive
- When a matching directory is found, its contents are not scanned individually
- Deleting from `/Library` may require `sudo`

## License

MIT. SEE: `LICENSE`
