//! Process configuration read once at startup.

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub database_url: String,
    pub pool_size: u32,
    pub request_timeout: Duration,
}

#[derive(Debug)]
pub enum ConfigError {
    Missing(&'static str),
    Invalid { key: &'static str, value: String },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::Missing("DATABASE_URL"))?;
        let listen = read_or("LISTEN_ADDR", "0.0.0.0:8080")?;
        let pool_size = read_or("DB_POOL_SIZE", "10")?;
        let timeout_secs: u64 = read_or("REQUEST_TIMEOUT_SECS", "15")?;
        Ok(Self {
            listen,
            database_url,
            pool_size,
            request_timeout: Duration::from_secs(timeout_secs),
        })
    }
}

fn read_or<T: std::str::FromStr>(key: &'static str, default: &str) -> Result<T, ConfigError> {
    let value = env::var(key).unwrap_or_else(|_| default.to_string());
    value
        .parse()
        .map_err(|_| ConfigError::Invalid { key, value })
}
