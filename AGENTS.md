# Quality Check

Before committing, run the following commands and ensure all pass.
If any check fails, fix the issues and re-run.

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --lib -- --test-threads=1
```
