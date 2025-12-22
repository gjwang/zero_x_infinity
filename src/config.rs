use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub log_level: String,
    pub log_dir: String,
    pub log_file: String,
    pub use_json: bool,
    pub rotation: String,
    pub sample_rate: usize,
    pub enable_tracing: bool,
    pub gateway: GatewayConfig,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    /// PostgreSQL connection URL for account management (Phase 0x0A)
    #[serde(default)]
    pub postgres_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub queue_size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PersistenceConfig {
    pub enabled: bool,
    pub tdengine_dsn: String,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tdengine_dsn: "taos://root:taosdata@localhost:6030".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load(env: &str) -> Self {
        let config_path = format!("config/{}.yaml", env);
        let content = fs::read_to_string(&config_path)
            .unwrap_or_else(|_| panic!("Failed to read config file: {}", config_path));
        serde_yaml::from_str(&content).expect("Failed to parse config yaml")
    }
}
