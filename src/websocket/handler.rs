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
    pub user_id: Option<u64>,
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
    let user_id = params.user_id.unwrap_or(0); // 0 = Anonymous
    ws.on_upgrade(move |socket| handle_socket(socket, user_id, manager))
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
            if let Ok(json) = serde_json::to_string(&msg)
                && sender.send(Message::Text(json.into())).await.is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages (ping/pong, close)
    let tx_for_recv = tx.clone();
    let manager_for_task = manager.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Try to parse command
                    if let Ok(cmd) = serde_json::from_str::<super::messages::WsCommand>(&text) {
                        match cmd {
                            super::messages::WsCommand::Subscribe { args } => {
                                let mut subscribed = Vec::new();
                                for topic in &args {
                                    manager_for_task.subscribe(conn_id, topic.clone());
                                    subscribed.push(topic.clone());
                                }
                                let _ =
                                    tx_for_recv.send(WsMessage::Subscribed { topics: subscribed });
                            }
                            super::messages::WsCommand::Unsubscribe { args } => {
                                let mut unsubscribed = Vec::new();
                                for topic in &args {
                                    manager_for_task.unsubscribe(conn_id, topic);
                                    unsubscribed.push(topic.clone());
                                }
                                let _ = tx_for_recv.send(WsMessage::Unsubscribed {
                                    topics: unsubscribed,
                                });
                            }
                        }
                    } else if text.contains("\"type\"") && text.contains("\"ping\"") {
                        // Keep legacy ping support just in case
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
