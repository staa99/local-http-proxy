use crate::config::AppConfig;
use crate::server;

pub async fn handle_start_command() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::instance();
    println!("🚀 Starting proxy server on port {}...", config.port);
    println!("   Mode: {}", config.mode);
    println!("   Routes loaded: {}", config.routes.len());

    server::start_server().await
}
