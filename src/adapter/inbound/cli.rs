use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::application::init_agent::InitAgent as AppInitAgent;

#[derive(Parser, Debug)]
#[command(
    name = "cupola",
    about = "GitHub Issue-driven automation agent",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InitAgent {
    #[value(name = "claude-code")]
    ClaudeCode,
}

impl From<InitAgent> for AppInitAgent {
    fn from(value: InitAgent) -> Self {
        match value {
            InitAgent::ClaudeCode => AppInitAgent::ClaudeCode,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start the polling loop
    Start {
        /// Polling interval override (seconds)
        #[arg(long)]
        polling_interval_secs: Option<u64>,

        /// Log level override (trace, debug, info, warn, error)
        #[arg(long)]
        log_level: Option<String>,

        /// Config file path (default: .cupola/cupola.toml)
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,

        /// Run as a background daemon
        #[arg(short = 'd', long)]
        daemon: bool,

        /// Internal: run as the daemon child process (do not use directly)
        #[arg(long, hide = true)]
        daemon_child: bool,
    },
    /// Stop the background daemon
    Stop {
        /// Config file path (default: .cupola/cupola.toml)
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,
    },
    /// Bootstrap Cupola into the current repository
    Init {
        /// Agent runtime to install assets for
        #[arg(long, value_enum, default_value_t = InitAgent::ClaudeCode)]
        agent: InitAgent,
    },
    /// Show status of all issues
    Status,
    /// Run environment diagnostics
    Doctor {
        /// Config file path
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,
    },
    /// Remove worktrees and branches associated with Cancelled issues
    Cleanup {
        /// Config file path (default: .cupola/cupola.toml)
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,
    },
    /// Detect completed specs and guide `/cupola:spec-compress` execution
    Compress,
    /// Show log file contents
    Logs {
        /// Config file path
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,

        /// Follow log output in real-time (like tail -f)
        #[arg(short = 'f', long)]
        follow: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_start_with_defaults() {
        let cli = Cli::parse_from(["cupola", "start"]);
        match cli.command {
            Command::Start {
                polling_interval_secs,
                log_level,
                config,
                daemon,
                ..
            } => {
                assert!(polling_interval_secs.is_none());
                assert!(log_level.is_none());
                assert_eq!(config, PathBuf::from(".cupola/cupola.toml"));
                assert!(!daemon);
            }
            _ => panic!("expected Start command"),
        }
    }

    #[test]
    fn parse_start_with_overrides() {
        let cli = Cli::parse_from([
            "cupola",
            "start",
            "--polling-interval-secs",
            "30",
            "--log-level",
            "debug",
            "--config",
            "/custom/path.toml",
        ]);
        match cli.command {
            Command::Start {
                polling_interval_secs,
                log_level,
                config,
                daemon,
                ..
            } => {
                assert_eq!(polling_interval_secs, Some(30));
                assert_eq!(log_level.as_deref(), Some("debug"));
                assert_eq!(config, PathBuf::from("/custom/path.toml"));
                assert!(!daemon);
            }
            _ => panic!("expected Start command"),
        }
    }

    #[test]
    fn parse_start_with_daemon_flag() {
        let cli = Cli::parse_from(["cupola", "start", "-d"]);
        match cli.command {
            Command::Start { daemon, .. } => {
                assert!(daemon);
            }
            _ => panic!("expected Start command"),
        }
    }

    #[test]
    fn parse_start_with_daemon_long_flag() {
        let cli = Cli::parse_from(["cupola", "start", "--daemon"]);
        match cli.command {
            Command::Start { daemon, .. } => {
                assert!(daemon);
            }
            _ => panic!("expected Start command"),
        }
    }

    #[test]
    fn parse_stop_with_defaults() {
        let cli = Cli::parse_from(["cupola", "stop"]);
        match cli.command {
            Command::Stop { config } => {
                assert_eq!(config, PathBuf::from(".cupola/cupola.toml"));
            }
            _ => panic!("expected Stop command"),
        }
    }

    #[test]
    fn parse_stop_with_custom_config() {
        let cli = Cli::parse_from(["cupola", "stop", "--config", "/custom/path.toml"]);
        match cli.command {
            Command::Stop { config } => {
                assert_eq!(config, PathBuf::from("/custom/path.toml"));
            }
            _ => panic!("expected Stop command"),
        }
    }

    #[test]
    fn parse_init_command() {
        let cli = Cli::parse_from(["cupola", "init"]);
        assert!(matches!(
            cli.command,
            Command::Init {
                agent: InitAgent::ClaudeCode
            }
        ));
    }

    #[test]
    fn parse_init_with_agent() {
        let cli = Cli::parse_from(["cupola", "init", "--agent", "claude-code"]);
        assert!(matches!(
            cli.command,
            Command::Init {
                agent: InitAgent::ClaudeCode
            }
        ));
    }

    #[test]
    fn parse_status_command() {
        let cli = Cli::parse_from(["cupola", "status"]);
        assert!(matches!(cli.command, Command::Status));
    }

    #[test]
    fn parse_doctor_with_defaults() {
        let cli = Cli::parse_from(["cupola", "doctor"]);
        match cli.command {
            Command::Doctor { config } => {
                assert_eq!(config, PathBuf::from(".cupola/cupola.toml"));
            }
            _ => panic!("expected Doctor command"),
        }
    }

    #[test]
    fn parse_doctor_with_custom_config() {
        let cli = Cli::parse_from(["cupola", "doctor", "--config", "/custom/path.toml"]);
        match cli.command {
            Command::Doctor { config } => {
                assert_eq!(config, PathBuf::from("/custom/path.toml"));
            }
            _ => panic!("expected Doctor command"),
        }
    }

    #[test]
    fn parse_cleanup_with_defaults() {
        let cli = Cli::parse_from(["cupola", "cleanup"]);
        match cli.command {
            Command::Cleanup { config } => {
                assert_eq!(config, PathBuf::from(".cupola/cupola.toml"));
            }
            _ => panic!("expected Cleanup command"),
        }
    }

    #[test]
    fn parse_cleanup_with_custom_config() {
        let cli = Cli::parse_from(["cupola", "cleanup", "--config", "/custom/path.toml"]);
        match cli.command {
            Command::Cleanup { config } => {
                assert_eq!(config, PathBuf::from("/custom/path.toml"));
            }
            _ => panic!("expected Cleanup command"),
        }
    }

    #[test]
    fn parse_logs_with_defaults() {
        let cli = Cli::parse_from(["cupola", "logs"]);
        match cli.command {
            Command::Logs { config, follow } => {
                assert_eq!(config, PathBuf::from(".cupola/cupola.toml"));
                assert!(!follow);
            }
            _ => panic!("expected Logs command"),
        }
    }

    #[test]
    fn parse_logs_with_follow_short() {
        let cli = Cli::parse_from(["cupola", "logs", "-f"]);
        match cli.command {
            Command::Logs { follow, .. } => {
                assert!(follow);
            }
            _ => panic!("expected Logs command"),
        }
    }

    #[test]
    fn parse_logs_with_follow_long() {
        let cli = Cli::parse_from(["cupola", "logs", "--follow"]);
        match cli.command {
            Command::Logs { follow, .. } => {
                assert!(follow);
            }
            _ => panic!("expected Logs command"),
        }
    }

    #[test]
    fn version_long_flag_returns_display_version() {
        let err = Cli::try_parse_from(["cupola", "--version"])
            .expect_err("--version should not parse successfully");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        let msg = err.to_string();
        assert!(
            msg.contains("cupola"),
            "message should contain 'cupola': {msg}"
        );
        assert!(
            msg.contains(env!("CARGO_PKG_VERSION")),
            "message should contain version '{}': {msg}",
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn version_short_flag_returns_display_version() {
        let err =
            Cli::try_parse_from(["cupola", "-V"]).expect_err("-V should not parse successfully");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
        let long_err = Cli::try_parse_from(["cupola", "--version"])
            .expect_err("--version should not parse successfully");
        assert_eq!(
            err.to_string(),
            long_err.to_string(),
            "-V and --version should produce identical output"
        );
    }
}
