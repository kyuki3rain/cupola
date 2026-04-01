use clap::Parser;
use cupola::adapter::inbound::cli::Cli;
use cupola::bootstrap::app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let unused_var = 42; // intentional clippy failure for E2E test
    let cli = Cli::parse();
    app::run(cli).await
}
