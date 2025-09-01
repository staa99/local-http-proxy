mod config;
mod server;

use std::error::Error;
use config::AppConfig;

#[tokio::main]
async fn main()  -> Result<(), Box<dyn Error + Send + Sync>> {
    // This handles all CLI commands.
    // If the command is 'start', it loads config and returns the instance.
    // Otherwise, it performs the action (e.g., 'add') and exits.
    let config = AppConfig::load();

    // The following code only runs for the `start` command.
    println!("🚀 Starting proxy server on port {}...", config.port);
    println!("   Mode: {}", config.mode);
    println!("   Routes loaded: {}", config.routes.len());

    server::start_server().await
}
