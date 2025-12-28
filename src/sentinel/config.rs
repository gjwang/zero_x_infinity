use serde::Deserialize;

/// Main Sentinel service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct SentinelConfig {
    pub service: ServiceConfig,
    pub chains: ChainsConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub poll_interval_ms: u64,
    pub health_check_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChainsConfig {
    pub btc: Option<ChainRef>,
    pub eth: Option<ChainRef>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChainRef {
    pub enabled: bool,
    pub config_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

/// BTC chain-specific configuration
#[derive(Debug, Deserialize, Clone)]
pub struct BtcChainConfig {
    pub chain_id: String,
    pub network: String,
    pub rpc: BtcRpcConfig,
    pub scanning: ScanningConfig,
    pub health: HealthConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BtcRpcConfig {
    pub url: String,
    pub user: String,
    pub password: String,
}

/// ETH chain-specific configuration
#[derive(Debug, Deserialize, Clone)]
pub struct EthChainConfig {
    pub chain_id: String,
    pub network: String,
    pub rpc: EthRpcConfig,
    pub scanning: ScanningConfig,
    pub health: HealthConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EthRpcConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScanningConfig {
    pub required_confirmations: u32,
    pub max_reorg_depth: u32,
    #[serde(default)]
    pub start_height: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HealthConfig {
    pub max_block_lag_seconds: i64,
}

impl SentinelConfig {
    /// Load configuration from YAML file
    pub fn from_file(path: &str) -> Result<Self, super::error::SentinelError> {
        let content = std::fs::read_to_string(path)?;
        let config: SentinelConfig = serde_yaml::from_str(&content)
            .map_err(|e| super::error::SentinelError::Config(e.to_string()))?;
        Ok(config)
    }
}

impl BtcChainConfig {
    /// Load BTC chain config from YAML file
    pub fn from_file(path: &str) -> Result<Self, super::error::SentinelError> {
        let content = std::fs::read_to_string(path)?;
        let config: BtcChainConfig = serde_yaml::from_str(&content)
            .map_err(|e| super::error::SentinelError::Config(e.to_string()))?;
        Ok(config)
    }
}

impl EthChainConfig {
    /// Load ETH chain config from YAML file
    pub fn from_file(path: &str) -> Result<Self, super::error::SentinelError> {
        let content = std::fs::read_to_string(path)?;
        let config: EthChainConfig = serde_yaml::from_str(&content)
            .map_err(|e| super::error::SentinelError::Config(e.to_string()))?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TDD (RED): Test config deserialization
    #[test]
    fn test_sentinel_config_deserialize() {
        let yaml = r#"
service:
  name: "sentinel"
  poll_interval_ms: 5000
  health_check_interval_ms: 30000
chains:
  btc:
    enabled: true
    config_path: "./config/chains/btc_regtest.yaml"
  eth:
    enabled: false
    config_path: "./config/chains/eth_anvil.yaml"
database:
  url: "postgres://localhost/test"
"#;

        let config: SentinelConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.service.name, "sentinel");
        assert_eq!(config.service.poll_interval_ms, 5000);
        assert!(config.chains.btc.as_ref().unwrap().enabled);
        assert!(!config.chains.eth.as_ref().unwrap().enabled);
    }

    #[test]
    fn test_btc_chain_config_deserialize() {
        let yaml = r#"
chain_id: "BTC"
network: "regtest"
rpc:
  url: "http://127.0.0.1:18443"
  user: "user"
  password: "pass"
scanning:
  required_confirmations: 3
  max_reorg_depth: 10
  start_height: 0
health:
  max_block_lag_seconds: 3600
"#;

        let config: BtcChainConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.chain_id, "BTC");
        assert_eq!(config.network, "regtest");
        assert_eq!(config.scanning.required_confirmations, 3);
        assert_eq!(config.health.max_block_lag_seconds, 3600);
    }

    #[test]
    fn test_eth_chain_config_deserialize() {
        let yaml = r#"
chain_id: "ETH"
network: "anvil"
rpc:
  url: "http://127.0.0.1:8545"
scanning:
  required_confirmations: 1
  max_reorg_depth: 10
health:
  max_block_lag_seconds: 3600
"#;

        let config: EthChainConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.chain_id, "ETH");
        assert_eq!(config.network, "anvil");
        assert_eq!(config.scanning.required_confirmations, 1);
    }
}
