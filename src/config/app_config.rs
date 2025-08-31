use super::args::{Args, Command};
use super::models::ProxyMode;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process;
use std::sync::OnceLock;

static CONFIG: OnceLock<AppConfig> = OnceLock::new();

// Represents the structure of the config.json file on disk.
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
struct ConfigFile {
    mode: ProxyMode,
    routes: HashMap<String, String>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            mode: ProxyMode::Path,
            routes: HashMap::new(),
        }
    }
}

/// Represents the active, in-memory configuration for the running application.
#[derive(Debug)]
pub struct AppConfig {
    pub port: u16,
    pub mode: ProxyMode,
    pub routes: HashMap<String, String>,
}

impl AppConfig {
    /// Initializes the AppConfig singleton by parsing CLI args and the config file.
    pub fn load() -> &'static AppConfig {
        let args = Args::parse();
        let expanded_path = shellexpand::tilde(&args.config_file);
        let config_path = Path::new(expanded_path.as_ref()).to_path_buf();

        match &args.command {
            Command::Start { .. } => handle_start_command(args, &config_path),
            _ => {
                // Handle commands that modify the config file and then terminate.
                if let Err(e) = handle_config_command(&args.command, &config_path) {
                    eprintln!("\nError:\n{}\n", e);
                    process::exit(1);
                }
                process::exit(0);
            }
        }
    }

    /// Retrieves the AppConfig singleton instance.
    /// Panics if `load()` has not been called yet.
    pub fn instance() -> &'static AppConfig {
        CONFIG.get().expect("AppConfig is not initialized!")
    }
}

/// Handles non-server commands (`list`, `add`, `remove`, `set-mode`).
fn handle_config_command(command: &Command, path: &Path) -> Result<(), Box<dyn Error>> {
    let mut config = read_config_file(path).unwrap_or_default();

    match command {
        Command::List => {
            println!("Mode: {}", config.mode);
            println!("Routes:");
            if config.routes.is_empty() {
                println!("  (No routes configured. Use the `add` command to create one.)");
            } else {
                let mut sorted_routes: Vec<_> = config.routes.iter().collect();
                sorted_routes.sort_by(|a, b| a.0.cmp(b.0));
                for (source, target) in sorted_routes {
                    println!("  {} → {}", source, target);
                }
            }
        }
        Command::Add { source, target } => {
            if !is_valid_source_name(source) {
                return Err(Box::from(format!(
                    "Invalid source name: \"{}\".\n\n  The name must be a single segment usable in both a URL path and a domain.\n\n  - Must contain only letters (a-z), numbers (0-9), and hyphens (-).\n  - Must not start or end with a hyphen.\n  - Must not contain '.' or '/'.\n\n  Examples of valid names: 'my-app', 'api', 'project1'",
                    source
                )));
            }

            if let Some(old) = config.routes.insert(source.clone(), target.clone()) {
                println!("✅ Updated route: {} → {} (was → {})", source, target, old);
            } else {
                println!("✅ Added route: {} → {}", source, target);
            }
            write_config_file(path, &config)?;
        }
        Command::Remove { source } => {
            if config.routes.remove(source).is_some() {
                println!("✅ Removed route for: {}", source);
                write_config_file(path, &config)?;
            } else {
                println!("⚠️  No route found for '{}'. Nothing to remove.", source);
            }
        }
        Command::SetMode { mode } => {
            config.mode = *mode;
            println!("✅ Proxy mode set to: {}", mode);
            write_config_file(path, &config)?;
        }
        Command::Start { .. } => unreachable!(),
    }
    Ok(())
}

fn handle_start_command(args: Args, path: &Path) -> &'static AppConfig {
    let port = if let Command::Start { port } = args.command {
        port
    } else {
        unreachable!(); // Guarded by the check above.
    };

    let file_content = load_or_create_config_file(path).unwrap_or_else(|e| {
        eprintln!("Error: Could not load configuration file.\n  Cause: {}", e);
        process::exit(1);
    });

    let config = AppConfig {
        port,
        mode: file_content.mode,
        routes: file_content.routes,
    };

    CONFIG
        .set(config)
        .expect("AppConfig should only be initialized once.");
    AppConfig::instance()
}

/// Validates that a source name is a valid DNS label and URL path segment.
fn is_valid_source_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    // Must not start or end with a hyphen (DNS label rule).
    let first_char_ok = name.chars().next().map_or(false, |c| c.is_alphanumeric());
    let last_char_ok = name.chars().last().map_or(false, |c| c.is_alphanumeric());

    if !first_char_ok || !last_char_ok {
        return false;
    }

    // Must only contain alphanumeric characters or hyphens.
    // This check also implicitly forbids '.' and '/'.
    name.chars().all(|c| c.is_alphanumeric() || c == '-')
}

/// Reads and parses the JSON config file from a given path.
fn read_config_file(path: &Path) -> Result<ConfigFile, Box<dyn Error>> {
    let content = fs::read_to_string(path)?;
    let config: ConfigFile = serde_json::from_str(&content).map_err(|e| {
        format!(
            "Configuration file at '{}' is invalid.\n  Details: {}",
            path.display(),
            e
        )
    })?;
    Ok(config)
}

/// Writes the given ConfigFile struct to a JSON file at the specified path.
fn write_config_file(path: &Path, config: &ConfigFile) -> Result<(), Box<dyn Error>> {
    // Create parent directory if it doesn't exist.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

/// Ensures a config file exists, creating a default one if needed, then reads it.
fn load_or_create_config_file(path: &Path) -> Result<ConfigFile, Box<dyn Error>> {
    if !path.exists() {
        let config = ConfigFile::default();
        write_config_file(path, &config)?;
        println!("Created a new default config file at: {}", path.display());
        return Ok(config);
    }
    read_config_file(path)
}
