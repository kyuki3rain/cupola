# Quality Check

Before committing, run the following commands and ensure all pass.
If any check fails, fix the issues and re-run.

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --lib -- --test-threads=1
```

# Design References

設計・実装判断を行う際は、[`docs/adr/`](docs/adr/) に格納された ADR（Architecture Decision Records）を必ず参照すること。
過去の意思決定の経緯・却下した代替案・根拠が記録されており、同じ議論を繰り返すことなく一貫した判断が行える。
