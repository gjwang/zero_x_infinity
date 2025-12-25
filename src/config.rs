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
    /// Matching Service Persistence (Phase 0x0D)
    #[serde(default)]
    pub matching_persistence: MatchingPersistenceConfig,
    /// Settlement Service Persistence (Phase 0x0D)
    #[serde(default)]
    pub settlement_persistence: SettlementPersistenceConfig,
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

/// Matching Service Persistence Configuration (Phase 0x0D)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MatchingPersistenceConfig {
    pub enabled: bool,
    pub data_dir: String,
    pub snapshot_interval_trades: u64,
}

impl Default for MatchingPersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_dir: "./data/matching".to_string(),
            snapshot_interval_trades: 1000,
        }
    }
}

/// Settlement Service Persistence Configuration (Phase 0x0D)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SettlementPersistenceConfig {
    pub enabled: bool,
    pub data_dir: String,
    pub checkpoint_interval: u64,
    pub snapshot_interval: u64,
}

impl Default for SettlementPersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_dir: "./data/settlement".to_string(),
            checkpoint_interval: 1000,
            snapshot_interval: 10000,
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
