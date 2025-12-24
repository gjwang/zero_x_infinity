use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;

use crate::symbol_manager::SymbolManager;

/// Configuration reload result
pub enum ConfigReloadResult {
    /// Config reloaded successfully
    Success,
    /// Config file read failed, keeping old config
    ReadError(String),
    /// Config validation failed, keeping old config
    ValidationError(String),
}

/// Validate symbol manager configuration
fn validate_config(manager: &SymbolManager) -> Result<(), String> {
    // Ensure at least one symbol is configured
    if manager.symbol_count() == 0 {
        return Err("No symbols configured".to_string());
    }

    // All symbols must have valid asset references
    for (id, info) in manager.iter_symbols() {
        if manager.get_asset_decimal(info.base_asset_id).is_none() {
            return Err(format!(
                "Symbol {} (id={}) references unknown base_asset_id={}",
                info.symbol, id, info.base_asset_id
            ));
        }
        if manager.get_asset_decimal(info.quote_asset_id).is_none() {
            return Err(format!(
                "Symbol {} (id={}) references unknown quote_asset_id={}",
                info.symbol, id, info.quote_asset_id
            ));
        }
    }

    Ok(())
}

/// Reload configuration from files
fn reload_config(_config_path: &Path) -> Result<SymbolManager, String> {
    // Use existing csv_io loading logic
    match std::panic::catch_unwind(crate::csv_io::load_symbol_manager) {
        Ok((manager, _active_symbol_id)) => Ok(manager),
        Err(_) => Err("Config loading panicked".to_string()),
    }
}

/// Background config watcher for hot-reload
///
/// Key principles:
/// - Never crash on config errors
/// - Keep old config if new config is invalid
/// - Log errors for monitoring/alerting
pub async fn config_watcher(
    config_path: &Path,
    manager: Arc<RwLock<SymbolManager>>,
    check_interval_secs: u64,
) {
    let config_path = config_path.to_path_buf();

    loop {
        tokio::time::sleep(Duration::from_secs(check_interval_secs)).await;

        let result = match reload_config(&config_path) {
            Ok(new_mgr) => {
                // Validate new configuration
                if let Err(e) = validate_config(&new_mgr) {
                    tracing::error!(
                        target: "CONFIG",
                        "Invalid config: {}, keeping old configuration",
                        e
                    );
                    // metrics::counter!("config_reload_error").increment(1);
                    ConfigReloadResult::ValidationError(e)
                } else {
                    // Atomic update
                    *manager.write().await = new_mgr;
                    tracing::info!(target: "CONFIG", "Config reloaded successfully");
                    ConfigReloadResult::Success
                }
            }
            Err(e) => {
                tracing::error!(
                    target: "CONFIG",
                    "Failed to reload config: {}, keeping old configuration",
                    e
                );
                // metrics::counter!("config_reload_error").increment(1);
                ConfigReloadResult::ReadError(e)
            }
        };

        // Log result for monitoring
        match result {
            ConfigReloadResult::Success => {}
            ConfigReloadResult::ReadError(_) | ConfigReloadResult::ValidationError(_) => {
                // In production: trigger PagerDuty alert here
                // pagerduty::trigger_alert("config_reload_failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_config() {
        let manager = SymbolManager::new();
        assert!(validate_config(&manager).is_err());
    }
}
