mod commands;
mod config;
mod server;

use crate::commands::{handle_config_command, handle_start_command};
use crate::config::Command;
use clap::Parser;
use config::{AppConfig, Args};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args = Args::parse();

    // ensure the app config is loaded and ready to be used in commands
    AppConfig::load(&args);

    match &args.command {
        Command::Start { .. } => handle_start_command().await,
        Command::List => handle_config_command(&args.command),
        Command::Add { .. } => handle_config_command(&args.command),
        Command::Remove { .. } => handle_config_command(&args.command),
        Command::SetMode { .. } => handle_config_command(&args.command),
    }
}
