use super::args::{Args, Command};
use super::models::ProxyMode;
use super::util::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Represents the active, in-memory configuration for the running application.
#[derive(Debug)]
pub struct AppConfig {
    pub path: PathBuf,
    pub port: u16,
    pub mode: ProxyMode,
    pub routes: HashMap<String, String>,
}

impl AppConfig {
    /// Initializes the AppConfig singleton by parsing CLI args and the config file.
    pub fn load(args: &Args) -> &'static AppConfig {
        let expanded_path = shellexpand::tilde(&args.config_file);
        let config_path = Path::new(expanded_path.as_ref()).to_path_buf();

        let file_content = load_or_create_config_file(&config_path).unwrap_or_else(|e| {
            eprintln!("Error: Could not load configuration file.\n  Cause: {}", e);
            process::exit(1);
        });

        let mut config = AppConfig {
            path: config_path,
            port: file_content.port,
            mode: file_content.mode,
            routes: file_content.routes,
        };

        apply_overrides(&mut config, args);
        CONFIG
            .set(config)
            .expect("AppConfig should only be initialized once.");
        AppConfig::instance()
    }

    /// Retrieves the AppConfig singleton instance.
    /// Panics if `load()` has not been called yet.
    pub fn instance() -> &'static AppConfig {
        CONFIG.get().expect("AppConfig is not initialized!")
    }
}

fn apply_overrides(config: &mut AppConfig, args: &Args) {
    match &args.command {
        Command::Start { port } => {
            config.port = *port;
        }
        _ => {
            // there's no overrides from the other commands yet
        }
    }
}
