# Quality Check

Before committing, run the following commands and ensure all pass.
If any check fails, fix the issues and re-run.

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --lib -- --test-threads=1
```

# Design References

When making design or implementation decisions, always consult the ADRs (Architecture Decision Records) stored in [`docs/adr/`](docs/adr/).
They document the background of past decisions, rejected alternatives, and rationale so that consistent decisions can be made without repeating the same discussions.
