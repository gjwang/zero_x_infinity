//! WebSocket module for real-time push notifications
//!
//! This module provides WebSocket support for pushing order updates,
//! trade notifications, and balance changes to connected clients.

pub mod connection;
pub mod handler;
pub mod messages;
pub mod ws_broadcast_service;

pub use connection::ConnectionManager;
pub use handler::ws_handler;
pub use messages::{PushEvent, WsMessage};
pub use ws_broadcast_service::WsService;
