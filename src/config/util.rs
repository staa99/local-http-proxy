use super::models::ConfigFile;
use std::error::Error;
use std::fs;
use std::path::Path;

/// Reads and parses the JSON config file from a given path.
pub fn read_config_file(path: &Path) -> Result<ConfigFile, Box<dyn Error>> {
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
pub fn write_config_file(path: &Path, config: &ConfigFile) -> Result<(), Box<dyn Error>> {
    // Create parent directory if it doesn't exist.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

/// Ensures a config file exists, creating a default one if needed, then reads it.
pub fn load_or_create_config_file(path: &Path) -> Result<ConfigFile, Box<dyn Error>> {
    if !path.exists() {
        let config = ConfigFile::default();
        write_config_file(path, &config)?;
        println!("Created a new default config file at: {}", path.display());
        return Ok(config);
    }
    read_config_file(path)
}
