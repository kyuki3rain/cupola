use anyhow::Result;

/// 外部コマンド実行の結果
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// コマンドが成功したか（exit code 0）
    pub success: bool,
    /// 標準出力
    pub stdout: String,
    /// 標準エラー出力
    pub stderr: String,
}

/// 外部コマンド実行の抽象化ポート
pub trait CommandRunner: Send + Sync {
    /// プログラム名と引数を受け取り、実行結果を返す。
    /// コマンドが存在しない場合も `CommandOutput { success: false, ... }` を返す（パニックしない）
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput>;
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::collections::HashMap;

    /// テスト用モック CommandRunner
    pub struct MockCommandRunner {
        /// (program, args) をキーとした事前設定レスポンス
        responses: HashMap<String, CommandOutput>,
        /// デフォルトレスポンス（キーに存在しない場合）
        default: CommandOutput,
    }

    impl MockCommandRunner {
        pub fn new() -> Self {
            Self {
                responses: HashMap::new(),
                default: CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "command not found".to_string(),
                },
            }
        }

        /// 特定コマンドに対するレスポンスを設定する
        pub fn with_success(mut self, program: &str, args: &[&str], stdout: &str) -> Self {
            let key = Self::make_key(program, args);
            self.responses.insert(
                key,
                CommandOutput {
                    success: true,
                    stdout: stdout.to_string(),
                    stderr: String::new(),
                },
            );
            self
        }

        pub fn with_failure(mut self, program: &str, args: &[&str]) -> Self {
            let key = Self::make_key(program, args);
            self.responses.insert(
                key,
                CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "error".to_string(),
                },
            );
            self
        }

        fn make_key(program: &str, args: &[&str]) -> String {
            format!("{} {}", program, args.join(" "))
        }
    }

    impl Default for MockCommandRunner {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CommandRunner for MockCommandRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            let key = Self::make_key(program, args);
            Ok(self
                .responses
                .get(&key)
                .cloned()
                .unwrap_or_else(|| self.default.clone()))
        }
    }
}
