use super::models::ProxyMode;
use clap::{Parser, Subcommand};

/// A simple local HTTP proxy for routing requests based on hostname or path.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the configuration file.
    #[arg(short, long, env, default_value = "~/.local-http-proxy/config.json")]
    pub config_file: String,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Starts the HTTP proxy server.
    Start {
        /// A custom port to override the main port argument for this command.
        #[arg(short, long, env, default_value_t = 8000)]
        port: u16,
    },

    /// Lists all active routes and the current mode.
    List,

    /// Adds a new route to the configuration.
    Add {
        /// The source host or path to match (e.g., my-app.local or /my-app).
        #[arg(index = 1)]
        source: String,
        /// The target server to forward to (e.g., localhost:3000).
        #[arg(index = 2)]
        target: String,
    },

    /// Removes an existing route from the configuration.
    Remove {
        /// The source host or path of the route to remove.
        #[arg(index = 1)]
        source: String,
    },

    /// Sets the proxy mode (`domain` or `path`).
    SetMode {
        /// The proxy mode to use.
        #[arg(index = 1)]
        mode: ProxyMode,
    },
}
