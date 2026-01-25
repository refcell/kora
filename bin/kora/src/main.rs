//! Kora - A minimal commonware + revm execution client.

use clap::Parser;
use eyre::Result;

#[derive(Parser, Debug)]
#[command(name = "kora")]
#[command(about = "A minimal commonware + revm execution client")]
struct Args {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    kora_cli::Backtracing::enable();
    #[cfg(unix)]
    kora_cli::SigsegvHandler::install();

    let args = Args::parse();

    tracing_subscriber::fmt().with_env_filter(if args.verbose { "debug" } else { "info" }).init();

    tracing::info!("Starting kora");

    Ok(())
}
