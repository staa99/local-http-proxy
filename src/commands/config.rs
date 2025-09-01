use super::util::{normalize_source_key, normalize_target};
use crate::config::{
    util::{read_config_file, write_config_file}, AppConfig, Command, ConfigFile,
    ProxyMode,
};
use std::error::Error;
use std::path::Path;
use std::process;

/// Handles non-server commands (`list`, `add`, `remove`, `set-mode`).
/// This function will exit the process after handling the command.
pub fn handle_config_command(command: &Command) -> ! {
    let config = AppConfig::instance();
    match handle_config_command_with_error_capture(command, &config.path) {
        Ok(..) => {
            process::exit(0);
        }
        Err(e) => {
            eprintln!("\nError:\n{}\n", e);
            process::exit(1);
        }
    }
}

/// Handles non-server commands (`list`, `add`, `remove`, `set-mode`).
/// Returns a Result to capture errors without exiting the process.
fn handle_config_command_with_error_capture(
    command: &Command,
    path: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut config = read_config_file(path).unwrap_or_default();

    match command {
        Command::List => {
            handle_list_command(&mut config);
        }
        Command::Add { source, target } => {
            handle_add_command(path, &mut config, source, target)?;
        }
        Command::Remove { source } => {
            handle_remove_command(path, &mut config, source)?;
        }
        Command::SetMode { mode } => {
            handle_set_mode_command(path, &mut config, mode)?;
        }
        Command::Start { .. } => unreachable!(),
    }
    Ok(())
}

fn handle_list_command(config: &mut ConfigFile) {
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

fn handle_add_command(
    path: &Path,
    config: &mut ConfigFile,
    source: &String,
    target: &String,
) -> Result<(), Box<dyn Error>> {
    let source = normalize_source_key(source).map_err(|e| Box::<dyn Error>::from(e))?;
    let target = normalize_target(target).map_err(|e| Box::<dyn Error>::from(e))?;

    let source_to_target = format!("{source} → {target}");
    if let Some(old) = config.routes.insert(source, target) {
        println!("✅ Updated route: {source_to_target} (was → {old})");
    } else {
        println!("✅ Added route: {source_to_target}");
    }
    write_config_file(path, &config)?;
    Ok(())
}

fn handle_remove_command(
    path: &Path,
    config: &mut ConfigFile,
    source: &String,
) -> Result<(), Box<dyn Error>> {
    let source = match normalize_source_key(source) {
        Ok(s) => s,
        Err(_) => source.trim().to_ascii_lowercase(),
    };

    if config.routes.remove(&source).is_some() {
        println!("✅ Removed route for: {source}");
        write_config_file(path, &config)?;
    } else {
        println!("⚠️  No route found for '{source}'. Nothing to remove.");
    }
    Ok(())
}

fn handle_set_mode_command(
    path: &Path,
    config: &mut ConfigFile,
    mode: &ProxyMode,
) -> Result<(), Box<dyn Error>> {
    config.mode = *mode;
    println!("✅ Proxy mode set to: {}", mode);
    write_config_file(path, &config)?;
    Ok(())
}
