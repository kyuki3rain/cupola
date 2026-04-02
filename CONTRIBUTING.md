# Contributing to Cupola

Thank you for your interest in contributing to Cupola! This guide will help you get started.

## Prerequisites

| Tool | Purpose |
|------|---------|
| [Rust](https://www.rust-lang.org/) stable | Build |
| [devbox](https://www.jetify.com/devbox) | Development environment (Nix-based) |
| [gh CLI](https://cli.github.com/) | GitHub API operations |
| Git | Version control |

## Development Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/kyuki3rain/cupola.git
   cd cupola
   ```

2. Enter the development environment:

   ```bash
   devbox shell
   ```

   This sets up all required tools (Rust, etc.) at once. If you prefer not to use devbox, install Rust stable manually.

3. Build and run tests:

   ```bash
   cargo build
   cargo test
   ```

## How to Contribute

### Reporting Bugs

Please use the [Bug Report](https://github.com/kyuki3rain/cupola/issues/new?template=bug_report.yml) issue template.

### Suggesting Features

Please use the [Feature Request](https://github.com/kyuki3rain/cupola/issues/new?template=feature_request.yml) issue template.

### Submitting Pull Requests

1. Create a feature branch from `main`:

   ```bash
   git checkout -b feat/your-feature
   ```

2. Make your changes and ensure all checks pass (see [Coding Standards](#coding-standards))

3. Commit using [Conventional Commits](https://www.conventionalcommits.org/):

   ```
   feat: add new feature
   fix: resolve bug in polling
   docs: update README
   refactor: simplify state machine
   test: add integration tests
   chore: update dependencies
   ```

4. Push and open a pull request against `main`

5. All CI checks must pass before merge

## Coding Standards

Before committing, always run:

```bash
cargo fmt           # Format code
cargo clippy --all-targets  # Lint
cargo test          # Run all tests
```

All three must pass. CI enforces these checks on every pull request.

## Architecture

Cupola follows Clean Architecture with 4 layers. See the [Architecture Overview](README.md#architecture-overview) in the README for details.

When contributing code, ensure dependencies point inward only: `domain` ← `application` ← `adapter` ← `bootstrap`.

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
