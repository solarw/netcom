mod client;
mod server;
mod protocol;
mod codec;
mod xauth;

use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;

/// A simple libp2p Echo application that can operate as both client and server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run as a server
    Server {
        /// Optional path to store the private key
        #[arg(short, long)]
        key_file: Option<PathBuf>,
    },
    /// Run as a client
    Client {
        /// The server URI to connect to
        #[arg(required = true)]
        server_uri: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Command::Server { key_file } => {
            server::run_server(key_file).await?;
        }
        Command::Client { server_uri } => {
            client::run_client(&server_uri).await?;
        }
    }

    Ok(())
}