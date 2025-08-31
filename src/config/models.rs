use clap::ValueEnum;
use serde::{Deserialize, Serialize};
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