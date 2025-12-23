//! Internal Transfer FSM
//!
//! Implements distributed 2-phase commit for Funding <-> Trading transfers.
//!
//! # Architecture
//!
//! The transfer module uses a Persistent FSM stored in PostgreSQL to coordinate
//! fund movements between two independent systems:
//! - **Funding Account** (PostgreSQL `balances_tb`)
//! - **Trading Account** (UBSCore RAM)
//!
//! # State Machine
//!
//! ```text
//! INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED
//!              ↓                              ↓
//!           FAILED                     COMPENSATING → ROLLED_BACK
//! ```
//!
//! # Safety Invariants
//!
//! 1. **Persist-Before-Call**: Always update DB state before calling external services
//! 2. **Explicit Fail Rule**: Only rollback on `EXPLICIT_FAIL`, never on timeout/unknown
//! 3. **Idempotency**: All adapter operations must be idempotent via `req_id`
//! 4. **Trading Cannot Rollback**: Once Trading withdraws, must retry target forever

pub mod adapters;
pub mod api;
pub mod coordinator;
pub mod db;
pub mod error;
pub mod state;
pub mod types;
pub mod worker;

// Re-exports for convenience
pub use api::{TransferApiRequest, TransferApiResponse, create_transfer_fsm, get_transfer_status};
pub use coordinator::TransferCoordinator;
pub use error::TransferError;
pub use state::TransferState;
pub use types::{OpResult, RequestId, ServiceId, TransferRecord, TransferRequest};
pub use worker::RecoveryWorker;
