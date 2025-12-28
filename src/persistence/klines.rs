//! K-Line Stream Computing
//!
//! Creates TDengine streams for automatic K-Line aggregation from trades table.

use anyhow::Result;
use taos::*;

/// Supported K-Line intervals
pub const KLINE_INTERVALS: &[(&str, &str)] = &[
    ("1m", "1m"),
    ("5m", "5m"),
    ("15m", "15m"),
    ("30m", "30m"),
    ("1h", "1h"),
    ("1d", "1d"),
];

/// Create K-Line streams for all intervals
///
/// Each stream aggregates trades into OHLCV data for a specific time window.
/// The streams write to klines subtables automatically using the `klines` super table.
pub async fn create_kline_streams(taos: &Taos) -> Result<()> {
    tracing::info!("Creating K-Line streams...");

    for (interval_name, interval_sql) in KLINE_INTERVALS {
        let stream_name = format!("kline_{}_stream", interval_name);
        // Subtable naming: kl_{interval}_{symbol_id} e.g., kl_5m_1
        let subtable_prefix = format!("kl_{}_", interval_name);

        // Use the unified `klines` super table with TAGS (symbol_id, intv)
        // The stream writes to subtables created from this super table
        let create_stream = format!(
            r#"
            CREATE STREAM IF NOT EXISTS {}
            INTO klines TAGS (symbol_id, '{}')
            SUBTABLE(CONCAT('{}', CAST(symbol_id AS VARCHAR(10))))
            AS SELECT
                _wstart AS ts,
                FIRST(price) AS open,
                MAX(price) AS high,
                MIN(price) AS low,
                LAST(price) AS close,
                SUM(qty) AS volume,
                SUM(CAST(price AS DOUBLE) * CAST(qty AS DOUBLE)) AS quote_volume,
                COUNT(*) AS trade_count
            FROM trades
            PARTITION BY symbol_id
            INTERVAL({})
            "#,
            stream_name, interval_name, subtable_prefix, interval_sql
        );

        match taos.exec(&create_stream).await {
            Ok(_) => {
                tracing::info!("Created stream: {}", stream_name);
            }
            Err(e) => {
                // Stream might already exist, log warning but continue
                tracing::warn!("Failed to create stream {}: {}", stream_name, e);
            }
        }
    }

    tracing::info!("K-Line streams setup complete");
    Ok(())
}

/// Drop all K-Line streams (for cleanup/rebuild)
#[allow(dead_code)]
pub async fn drop_kline_streams(taos: &Taos) -> Result<()> {
    for (interval_name, _) in KLINE_INTERVALS {
        let stream_name = format!("kline_{}_stream", interval_name);
        let drop_sql = format!("DROP STREAM IF EXISTS {}", stream_name);
        let _ = taos.exec(&drop_sql).await;
        tracing::info!("Dropped stream: {}", stream_name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_names() {
        assert_eq!(KLINE_INTERVALS.len(), 6);
        assert_eq!(KLINE_INTERVALS[0], ("1m", "1m"));
        assert_eq!(KLINE_INTERVALS[5], ("1d", "1d"));
    }
}
