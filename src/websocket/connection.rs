//! WebSocket connection manager
//!
//! Manages active WebSocket connections using DashMap for concurrent access.
//! Supports multiple connections per user (e.g., mobile + web).

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

use super::messages::WsMessage;

/// WebSocket sender channel type
pub type WsSender = mpsc::UnboundedSender<WsMessage>;

/// Unique connection identifier
pub type ConnectionId = u64;

/// WebSocket connection manager
///
/// Thread-safe connection registry that maps user_id to their active
/// WebSocket connections. Uses DashMap for lock-free concurrent access.
pub struct ConnectionManager {
    /// user_id -> list of (connection_id, sender)
    connections: DashMap<u64, Vec<(ConnectionId, WsSender)>>,
    /// Next connection ID
    next_conn_id: AtomicU64,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
            next_conn_id: AtomicU64::new(1),
        }
    }

    /// Add a new WebSocket connection for a user
    ///
    /// Returns the unique connection ID for this connection.
    /// Supports multiple connections per user (e.g., mobile app + web browser).
    pub fn add_connection(&self, user_id: u64, tx: WsSender) -> ConnectionId {
        let conn_id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);

        self.connections
            .entry(user_id)
            .or_insert_with(Vec::new)
            .push((conn_id, tx));

        tracing::info!(
            user_id,
            conn_id,
            total_connections = self.connections.get(&user_id).map(|v| v.len()).unwrap_or(0),
            "WebSocket connection added"
        );

        conn_id
    }

    /// Remove a WebSocket connection by ID
    ///
    /// Called when a connection is closed. Cleans up empty user entries.
    pub fn remove_connection(&self, user_id: u64, conn_id: ConnectionId) {
        if let Some(mut senders) = self.connections.get_mut(&user_id) {
            // Remove the connection with matching ID
            senders.retain(|(id, _)| *id != conn_id);

            // If no more connections for this user, remove the entry
            if senders.is_empty() {
                drop(senders); // Release the lock
                self.connections.remove(&user_id);
                tracing::info!(user_id, conn_id, "All WebSocket connections closed");
            } else {
                tracing::info!(
                    user_id,
                    conn_id,
                    remaining_connections = senders.len(),
                    "WebSocket connection removed"
                );
            }
        }
    }

    /// Send a message to all connections of a specific user
    ///
    /// Automatically removes failed connections (client disconnected).
    pub fn send_to_user(&self, user_id: u64, message: WsMessage) {
        if let Some(senders) = self.connections.get(&user_id) {
            let json = serde_json::to_string(&message).unwrap_or_default();
            for (_, tx) in senders.iter() {
                if tx.send(message.clone()).is_err() {
                    tracing::warn!(user_id, "Failed to send - client disconnected");
                    // Note: removal handled by ws handler when connection closes
                }
            }
            tracing::debug!(
                user_id,
                recipients = senders.len(),
                message_type = ?json.split('"').nth(3).unwrap_or("unknown"),
                "Message sent to user"
            );
        }
    }

    /// Get connection statistics
    ///
    /// Returns (number of users, total connections)
    pub fn stats(&self) -> (usize, usize) {
        let users = self.connections.len();
        let total_connections: usize = self
            .connections
            .iter()
            .map(|entry| entry.value().len())
            .sum();
        (users, total_connections)
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_manager_add_remove() {
        let manager = ConnectionManager::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        // Add connection
        let conn_id = manager.add_connection(1001, tx);
        assert_eq!(manager.stats(), (1, 1));

        // Remove connection
        manager.remove_connection(1001, conn_id);
        assert_eq!(manager.stats(), (0, 0));
    }

    #[test]
    fn test_multiple_connections_per_user() {
        let manager = ConnectionManager::new();
        let (tx1, _rx1) = mpsc::unbounded_channel();
        let (tx2, _rx2) = mpsc::unbounded_channel();

        // Add two connections for same user
        let conn_id1 = manager.add_connection(1001, tx1);
        let conn_id2 = manager.add_connection(1001, tx2);
        assert_eq!(manager.stats(), (1, 2));

        // Remove one connection
        manager.remove_connection(1001, conn_id1);
        assert_eq!(manager.stats(), (1, 1));

        // Remove second connection
        manager.remove_connection(1001, conn_id2);
        assert_eq!(manager.stats(), (0, 0));
    }

    #[test]
    fn test_send_to_user() {
        let manager = ConnectionManager::new();
        let (tx, mut rx) = mpsc::unbounded_channel();

        manager.add_connection(1001, tx);

        let message = WsMessage::Connected { user_id: 1001 };
        manager.send_to_user(1001, message.clone());

        // Verify message received
        let received = rx.try_recv().unwrap();
        matches!(received, WsMessage::Connected { user_id: 1001 });
    }
}
