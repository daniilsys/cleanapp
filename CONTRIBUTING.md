# Contributing to cleanapp

Thanks for your interest in contributing to cleanapp! Here's how to get started.

## Getting started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/daniilsys/cleanapp
   cd cleanapp
   ```
3. Make sure it builds:
   ```bash
   cargo build
   ```

## Development

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)

### Useful commands

```bash
cargo build              # Build debug
cargo build --release    # Build release
cargo run -- Spotify     # Run with args
cargo test               # Run tests
cargo clippy             # Lint
cargo fmt                # Format code
```

### Project structure

```
src/
├── main.rs          # CLI entry point, interactive menu, argument parsing
├── search.rs        # Core search algorithm (directory traversal, matching)
├── clean_files.rs   # File and directory deletion
└── get_results.rs   # Search path resolution, root selection per platform
```

## Making changes

1. Create a branch from `main`:
   ```bash
   git checkout -b my-feature
   ```
2. Make your changes
3. Make sure CI checks pass locally:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```
4. Commit with a clear message:
   ```bash
   git commit -m "Add support for ..."
   ```
5. Push and open a Pull Request against `main`

## Pull request guidelines

- Keep PRs focused on a single change
- Add tests if you're adding new functionality
- Make sure all CI checks pass
- Update the README if you're adding or changing CLI options

## Reporting bugs

Open an [issue](https://github.com/daniilsys/cleanapp/issues) with:

- Your OS and version
- The command you ran
- What you expected to happen
- What actually happened

## Suggesting features

Open an [issue](https://github.com/daniilsys/cleanapp/issues) with the `enhancement` label and describe your use case.

## Code style

- Follow standard Rust conventions
- Run `cargo fmt` before committing
- No clippy warnings (`cargo clippy -- -D warnings`)

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
