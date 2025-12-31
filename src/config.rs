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
    /// UBSCore Service Persistence (Phase 0x0D)
    #[serde(default)]
    pub ubscore_persistence: UBSCorePersistenceConfig,
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
            data_dir: "./data/matching-service".to_string(),
            snapshot_interval_trades: 1000,
        }
    }
}

/// UBSCore Service Persistence Configuration (Phase 0x0D)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UBSCorePersistenceConfig {
    pub enabled: bool,
    pub data_dir: String,
    pub wal_dir: String,
    pub snapshot_interval_orders: u64,
}

impl Default for UBSCorePersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_dir: "./data/ubscore".to_string(),
            wal_dir: "./data/ubscore/wal".to_string(),
            snapshot_interval_orders: 5000,
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
            data_dir: "./data/settlement-service".to_string(),
            checkpoint_interval: 1000,
            snapshot_interval: 10000,
        }
    }
}

use anyhow::{Context, Result};

impl AppConfig {
    /// Load config from YAML file based on environment
    pub fn load(env: &str) -> Result<Self> {
        let config_path = format!("config/{}.yaml", env);
        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path))?;
        let mut config: Self =
            serde_yaml::from_str(&content).context("Failed to parse config yaml")?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides
    ///
    /// Environment variables take precedence over YAML config.
    /// Format: ZXI_<SECTION>_<FIELD> (uppercase with underscores)
    ///
    /// Supported overrides:
    /// - ZXI_GATEWAY_PORT: Gateway port (u16)
    /// - ZXI_GATEWAY_HOST: Gateway host (String)
    /// - ZXI_POSTGRES_URL: PostgreSQL connection URL
    /// - ZXI_LOG_LEVEL: Log level (trace/debug/info/warn/error)
    /// - ZXI_PERSISTENCE_ENABLED: Enable TDengine persistence (true/false)
    /// - ZXI_PERSISTENCE_DSN: TDengine DSN
    pub fn apply_env_overrides(&mut self) {
        // Gateway overrides
        if let Ok(port) = std::env::var("ZXI_GATEWAY_PORT")
            && let Ok(p) = port.parse::<u16>()
        {
            tracing::info!(
                "Config override: gateway.port = {} (from ZXI_GATEWAY_PORT)",
                p
            );
            self.gateway.port = p;
        }
        if let Ok(host) = std::env::var("ZXI_GATEWAY_HOST") {
            tracing::info!(
                "Config override: gateway.host = {} (from ZXI_GATEWAY_HOST)",
                host
            );
            self.gateway.host = host;
        }

        // PostgreSQL override
        if let Ok(url) = std::env::var("ZXI_POSTGRES_URL") {
            tracing::info!("Config override: postgres_url = [REDACTED] (from ZXI_POSTGRES_URL)");
            self.postgres_url = Some(url);
        }

        // Logging overrides
        if let Ok(level) = std::env::var("ZXI_LOG_LEVEL") {
            tracing::info!(
                "Config override: log_level = {} (from ZXI_LOG_LEVEL)",
                level
            );
            self.log_level = level;
        }

        // Persistence overrides
        if let Ok(enabled) = std::env::var("ZXI_PERSISTENCE_ENABLED")
            && let Ok(e) = enabled.parse::<bool>()
        {
            tracing::info!(
                "Config override: persistence.enabled = {} (from ZXI_PERSISTENCE_ENABLED)",
                e
            );
            self.persistence.enabled = e;
        }
        if let Ok(dsn) = std::env::var("ZXI_PERSISTENCE_DSN") {
            tracing::info!(
                "Config override: persistence.tdengine_dsn = [REDACTED] (from ZXI_PERSISTENCE_DSN)"
            );
            self.persistence.tdengine_dsn = dsn;
        }
    }

    /// Validate configuration at startup
    ///
    /// Returns an error if any critical configuration is invalid.
    pub fn validate(&self) -> Result<()> {
        // Validate gateway port
        if self.gateway.port == 0 {
            anyhow::bail!("Invalid gateway.port: must be > 0");
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log_level.to_lowercase().as_str()) {
            anyhow::bail!(
                "Invalid log_level '{}': must be one of {:?}",
                self.log_level,
                valid_levels
            );
        }

        // Validate queue size
        if self.gateway.queue_size == 0 {
            anyhow::bail!("Invalid gateway.queue_size: must be > 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_config() {
        let config = AppConfig {
            log_level: "info".to_string(),
            log_dir: "./logs".to_string(),
            log_file: "app.log".to_string(),
            use_json: false,
            rotation: "daily".to_string(),
            sample_rate: 100,
            enable_tracing: false,
            gateway: GatewayConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                queue_size: 1024,
            },
            persistence: PersistenceConfig::default(),
            ubscore_persistence: UBSCorePersistenceConfig::default(),
            matching_persistence: MatchingPersistenceConfig::default(),
            settlement_persistence: SettlementPersistenceConfig::default(),
            postgres_url: None,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_port() {
        let config = AppConfig {
            log_level: "info".to_string(),
            log_dir: "./logs".to_string(),
            log_file: "app.log".to_string(),
            use_json: false,
            rotation: "daily".to_string(),
            sample_rate: 100,
            enable_tracing: false,
            gateway: GatewayConfig {
                host: "0.0.0.0".to_string(),
                port: 0, // Invalid
                queue_size: 1024,
            },
            persistence: PersistenceConfig::default(),
            ubscore_persistence: UBSCorePersistenceConfig::default(),
            matching_persistence: MatchingPersistenceConfig::default(),
            settlement_persistence: SettlementPersistenceConfig::default(),
            postgres_url: None,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let config = AppConfig {
            log_level: "invalid".to_string(), // Invalid
            log_dir: "./logs".to_string(),
            log_file: "app.log".to_string(),
            use_json: false,
            rotation: "daily".to_string(),
            sample_rate: 100,
            enable_tracing: false,
            gateway: GatewayConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                queue_size: 1024,
            },
            persistence: PersistenceConfig::default(),
            ubscore_persistence: UBSCorePersistenceConfig::default(),
            matching_persistence: MatchingPersistenceConfig::default(),
            settlement_persistence: SettlementPersistenceConfig::default(),
            postgres_url: None,
        };
        assert!(config.validate().is_err());
    }
}
