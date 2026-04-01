use clap::Parser;
use cupola::adapter::inbound::cli::Cli;
use cupola::bootstrap::app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    app::run(cli).await
}
