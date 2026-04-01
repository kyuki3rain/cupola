# Quality Check

commit 前に以下をすべて実行し、全てパスしてから commit すること。失敗した場合は修正して再チェックすること。

1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
