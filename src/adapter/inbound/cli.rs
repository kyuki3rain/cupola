use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "cupola", about = "GitHub Issue-driven automation agent")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
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
    /// Initialize the SQLite schema
    Init,
    /// Show status of all issues
    Status,
    /// Run environment diagnostics
    Doctor {
        /// Config file path
        #[arg(long, default_value = ".cupola/cupola.toml")]
        config: PathBuf,
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
        assert!(matches!(cli.command, Command::Init));
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
}
