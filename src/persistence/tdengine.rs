use anyhow::{Context, Result};
use taos::*;

/// TDengine client for persistence operations
pub struct TDengineClient {
    taos: Taos,
}

impl TDengineClient {
    /// Connect to TDengine using DSN
    ///
    /// # Example DSN
    /// ```
    /// taos+ws://root:taosdata@localhost:6041
    /// ```
    pub async fn connect(dsn: &str) -> Result<Self> {
        let builder = TaosBuilder::from_dsn(dsn)
            .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to parse TDengine DSN", e))?;

        let taos = builder
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to connect to TDengine", e))?;

        tracing::info!("Connected to TDengine: {}", dsn);

        Ok(Self { taos })
    }

    /// Initialize database schema
    pub async fn init_schema(&self) -> Result<()> {
        crate::persistence::schema::init_schema(&self.taos).await
    }

    /// Get reference to underlying Taos connection
    pub fn taos(&self) -> &Taos {
        &self.taos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_connect() {
        let client = TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
            .await
            .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");
    }
}
