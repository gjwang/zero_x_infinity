//! WebSocket handler for client connections
//!
//! Handles WebSocket upgrade, connection lifecycle, and message forwarding.

use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;

use super::connection::ConnectionManager;
use super::messages::WsMessage;
use crate::gateway::state::AppState;

/// WebSocket connection query parameters
#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub user_id: u64,
}

/// WebSocket upgrade handler
///
/// Endpoint: GET /ws?user_id=1001
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let manager = state.ws_manager.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, params.user_id, manager))
}

/// Handle WebSocket connection lifecycle
async fn handle_socket(socket: WebSocket, user_id: u64, manager: Arc<ConnectionManager>) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();

    // Register connection and get unique ID
    let conn_id = manager.add_connection(user_id, tx.clone());

    // Send welcome message
    let welcome = WsMessage::Connected { user_id };
    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Spawn task to forward messages from channel to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    let tx_for_recv = tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Handle ping
                    if text.contains("\"type\"") && text.contains("\"ping\"") {
                        let _ = tx_for_recv.send(WsMessage::Pong);
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Cleanup using connection ID
    manager.remove_connection(user_id, conn_id);
}
