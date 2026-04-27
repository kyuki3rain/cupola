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

## Adding a New Permission Template

Permission templates define which tools Claude Code subprocesses are allowed or denied to use.
Follow these steps to add a new built-in template.

### 1. Create the template file

Create `assets/claude-settings/<key>.json` with the following minimal structure:

```json
{
  "permissions": {
    "allow": ["Bash(<cmd>*)"]
  }
}
```

To also restrict tools, add a `deny` array:

```json
{
  "permissions": {
    "allow": ["Bash(<cmd>*)"],
    "deny": ["Bash(<restricted-cmd>*)"]
  }
}
```

Only list the operations that are *specific to this template*. The `base` template's entries
are always merged in automatically.

### 2. Register the template in `TEMPLATES`

Add an entry to the `TEMPLATES` constant in
`src/application/template_manager.rs`:

```rust
(
    "<key>",
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/claude-settings/<key>.json"
    )),
),
```

### 3. Add tests

In the `#[cfg(test)]` block of `src/application/template_manager.rs`, add at least two tests:

- **Single-template test**: verify that `build_settings(&["<key>"])` produces the expected
  allow entries.
- **Merge test**: verify that `build_settings(&["<key>", "<other>"])` includes entries from
  both templates.

```rust
#[test]
fn build_settings_with_<key>_template() {
    let settings = TemplateManager::build_settings(&["<key>"]).expect("build");
    assert!(
        settings.permissions.allow.iter().any(|a| a.contains("<cmd>")),
        "<key> template should add <cmd> commands"
    );
}

#[test]
fn build_settings_merges_<key>_and_base() {
    let settings = TemplateManager::build_settings(&["<key>", "rust"]).expect("build");
    assert!(settings.permissions.allow.iter().any(|a| a.contains("<cmd>")));
    assert!(settings.permissions.allow.iter().any(|a| a.contains("cargo")));
}
```

Run `cargo test` to confirm all tests pass.

### 4. Update the README template tables

Add the new template to the table in both:

- `README.md` — [Permission Templates](#permission-templates) section
- `README.ja.md` — `### Permission テンプレート一覧` section

### 5. Naming conventions

Choose a key that follows these conventions:

| Category | Examples | Rule |
|----------|---------|------|
| Language | `rust`, `python`, `go` | Lowercase language name |
| Ecosystem / tool | `devbox`, `docker` | Lowercase tool name |
| Framework | `nextjs`, `rails` | Lowercase framework name |

## Architecture

Cupola follows Clean Architecture with 4 layers. See the [Architecture Overview](README.md#architecture-overview) in the README for details.

When contributing code, ensure dependencies point inward only: `domain` ← `application` ← `adapter` ← `bootstrap`.

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).
