
use clap::{Parser, Subcommand};
use ddr::{client, manager, server};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as a download client
    Client(client::Args),
    /// Run as a download manager
    Manager(manager::Args),
    /// Run as a download server
    Server(server::Args),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Client(args) => {
            println!("Starting in client mode...");
            client::run(args).await?;
        }
        Commands::Manager(args) => {
            println!("Starting in manager mode...");
            manager::run(args).await?;
        }
        Commands::Server(args) => {
            println!("Starting in server mode...");
            server::run(args).await?;
        }
    }

    Ok(())
}
