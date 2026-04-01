# Quality Check

commit 前に以下をすべて実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。

1. `cargo fmt -- --check`
2. `RUSTFLAGS=-D warnings cargo clippy --all-targets`
3. `cargo test --lib -- --test-threads=1`
