use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result};

/// Defines the routing strategy for the proxy.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    /// Routes based on the request's hostname (e.g., `app.local`).
    Domain,
    /// Routes based on the request's path prefix (e.g., `/app`).
    Path,
}

impl Display for ProxyMode {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            ProxyMode::Domain => write!(f, "domain"),
            ProxyMode::Path => write!(f, "path"),
        }
    }
}

// Represents the structure of the config.json file on disk.
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct ConfigFile {
    pub port: u16,
    pub mode: ProxyMode,
    pub routes: HashMap<String, String>,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            port: 8000,
            mode: ProxyMode::Path,
            routes: HashMap::new(),
        }
    }
}
