//! Transfer Core Types
//!
//! Type definitions for the internal transfer FSM.

use std::fmt;
use std::str::FromStr;

pub use crate::money::ScaledAmount;

use super::state::TransferState;

/// Request ID type - ULID-based unique identifier
///
/// Using ULID provides:
/// - Monotonic, sortable IDs
/// - No coordination needed (no machine_id)
/// - 128-bit with good entropy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternalTransferId(ulid::Ulid);

impl InternalTransferId {
    /// Generate a new unique InternalTransferId
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    /// Get the inner ULID value
    pub fn inner(&self) -> ulid::Ulid {
        self.0
    }
}

impl Default for InternalTransferId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for InternalTransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for InternalTransferId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(ulid::Ulid::from_string(s)?))
    }
}

/// Service identifier for source/target of transfers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum ServiceId {
    /// Funding account (PostgreSQL `balances_tb`)
    Funding = 1,
    /// Trading account (UBSCore RAM)
    Trading = 2,
}

impl ServiceId {
    /// Get numeric ID for PostgreSQL storage
    #[inline]
    pub fn id(&self) -> i16 {
        *self as i16
    }

    /// Convert from PostgreSQL ID
    pub fn from_id(id: i16) -> Option<Self> {
        match id {
            1 => Some(ServiceId::Funding),
            2 => Some(ServiceId::Trading),
            _ => None,
        }
    }

    /// Get human-readable name
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceId::Funding => "FUNDING",
            ServiceId::Trading => "TRADING",
        }
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<i16> for ServiceId {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        ServiceId::from_id(value).ok_or(())
    }
}

/// Transfer type (direction)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum TransferType {
    /// Funding -> Spot (deposit to trading)
    FundingToSpot = 1,
    /// Spot -> Funding (withdraw from trading)
    SpotToFunding = 2,
}

impl TransferType {
    pub fn id(&self) -> i16 {
        *self as i16
    }

    pub fn from_id(id: i16) -> Option<Self> {
        match id {
            1 => Some(TransferType::FundingToSpot),
            2 => Some(TransferType::SpotToFunding),
            _ => None,
        }
    }

    /// Determine transfer type from source and target
    pub fn from_services(from: ServiceId, to: ServiceId) -> Option<Self> {
        match (from, to) {
            (ServiceId::Funding, ServiceId::Trading) => Some(TransferType::FundingToSpot),
            (ServiceId::Trading, ServiceId::Funding) => Some(TransferType::SpotToFunding),
            _ => None, // Same service transfers not allowed
        }
    }
}

/// Operation result from service adapters
#[derive(Debug, Clone)]
pub enum OpResult {
    /// Operation completed successfully
    Success,
    /// Operation failed with explicit error (safe to rollback)
    Failed(String),
    /// Operation state unknown (timeout, network error) - must retry
    Pending,
}

impl OpResult {
    /// Check if this is a success result
    #[inline]
    pub fn is_success(&self) -> bool {
        matches!(self, OpResult::Success)
    }

    /// Check if this is an explicit failure (safe to compensate)
    #[inline]
    pub fn is_explicit_fail(&self) -> bool {
        matches!(self, OpResult::Failed(_))
    }

    /// Check if state is unknown (must retry, NOT safe to compensate)
    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self, OpResult::Pending)
    }
}

/// Transfer request from API layer
#[derive(Debug, Clone)]
pub struct TransferRequest {
    /// Source account type
    pub from: ServiceId,
    /// Target account type
    pub to: ServiceId,
    /// User ID (must match authenticated user)
    pub user_id: u64,
    /// Asset ID
    pub asset_id: u32,
    /// Amount in scaled units (e.g., satoshis for BTC)
    pub amount: ScaledAmount,
    /// Client-provided idempotency key (optional)
    pub cid: Option<String>,
}

impl TransferRequest {
    /// Create a new transfer request
    pub fn new(
        from: ServiceId,
        to: ServiceId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
    ) -> Self {
        Self {
            from,
            to,
            user_id,
            asset_id,
            amount,
            cid: None,
        }
    }

    /// Create request with client idempotency key
    pub fn with_cid(
        from: ServiceId,
        to: ServiceId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
        cid: String,
    ) -> Self {
        Self {
            from,
            to,
            user_id,
            asset_id,
            amount,
            cid: Some(cid),
        }
    }

    /// Get the transfer type based on source and target
    pub fn transfer_type(&self) -> Option<TransferType> {
        TransferType::from_services(self.from, self.to)
    }
}

/// Transfer record stored in PostgreSQL
#[derive(Debug, Clone)]
pub struct TransferRecord {
    /// Unique transfer ID (ULID, also the DB primary key)
    pub transfer_id: InternalTransferId,
    /// Client idempotency key
    pub cid: Option<String>,
    /// Source service
    pub source: ServiceId,
    /// Target service
    pub target: ServiceId,
    /// Transfer type (derived from source/target)
    pub transfer_type: TransferType,
    /// User ID
    pub user_id: u64,
    /// Asset ID
    pub asset_id: u32,
    /// Amount in scaled units
    pub amount: ScaledAmount,
    /// Current FSM state
    pub state: TransferState,
    /// Last error message (for debugging)
    pub error: Option<String>,
    /// Retry count
    pub retry_count: i32,
    /// Created timestamp (millis)
    pub created_at: i64,
    /// Last updated timestamp (millis)
    pub updated_at: i64,
}

impl TransferRecord {
    /// Create a new transfer record in INIT state
    pub fn new(
        transfer_id: InternalTransferId,
        source: ServiceId,
        target: ServiceId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
        cid: Option<String>,
    ) -> Self {
        let transfer_type =
            TransferType::from_services(source, target).expect("Invalid source/target combination");
        let now = chrono::Utc::now().timestamp_millis();

        Self {
            transfer_id,
            cid,
            source,
            target,
            transfer_type,
            user_id,
            asset_id,
            amount,
            state: TransferState::Init,
            error: None,
            retry_count: 0,
            created_at: now,
            updated_at: now,
        }
    }
}

impl fmt::Display for TransferRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Transfer[{}] {} -> {} user={} asset={} amount={} state={}",
            self.transfer_id,
            self.source,
            self.target,
            self.user_id,
            self.asset_id,
            self.amount,
            self.state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id_roundtrip() {
        assert_eq!(ServiceId::from_id(1), Some(ServiceId::Funding));
        assert_eq!(ServiceId::from_id(2), Some(ServiceId::Trading));
        assert_eq!(ServiceId::from_id(0), None);
        assert_eq!(ServiceId::from_id(3), None);
    }

    #[test]
    fn test_transfer_type_from_services() {
        assert_eq!(
            TransferType::from_services(ServiceId::Funding, ServiceId::Trading),
            Some(TransferType::FundingToSpot)
        );
        assert_eq!(
            TransferType::from_services(ServiceId::Trading, ServiceId::Funding),
            Some(TransferType::SpotToFunding)
        );
        assert_eq!(
            TransferType::from_services(ServiceId::Funding, ServiceId::Funding),
            None
        );
    }

    #[test]
    fn test_op_result() {
        assert!(OpResult::Success.is_success());
        assert!(!OpResult::Success.is_explicit_fail());
        assert!(!OpResult::Success.is_pending());

        let fail = OpResult::Failed("test".to_string());
        assert!(!fail.is_success());
        assert!(fail.is_explicit_fail());
        assert!(!fail.is_pending());

        assert!(!OpResult::Pending.is_success());
        assert!(!OpResult::Pending.is_explicit_fail());
        assert!(OpResult::Pending.is_pending());
    }

    #[test]
    fn test_transfer_request() {
        let req = TransferRequest::new(
            ServiceId::Funding,
            ServiceId::Trading,
            1001,
            1,
            1000000.into(),
        );
        assert_eq!(req.transfer_type(), Some(TransferType::FundingToSpot));
        assert!(req.cid.is_none());

        let req_with_cid = TransferRequest::with_cid(
            ServiceId::Trading,
            ServiceId::Funding,
            1001,
            1,
            500000.into(),
            "client-123".to_string(),
        );
        assert_eq!(
            req_with_cid.transfer_type(),
            Some(TransferType::SpotToFunding)
        );
        assert_eq!(req_with_cid.cid, Some("client-123".to_string()));
    }

    #[test]
    fn test_transfer_record_new() {
        let transfer_id = InternalTransferId::new();
        let record = TransferRecord::new(
            transfer_id,
            ServiceId::Funding,
            ServiceId::Trading,
            1001,
            1,
            1000000.into(),
            None,
        );

        assert_eq!(record.transfer_id, transfer_id);
        assert_eq!(record.source, ServiceId::Funding);
        assert_eq!(record.target, ServiceId::Trading);
        assert_eq!(record.state, TransferState::Init);
        assert_eq!(record.retry_count, 0);
        assert!(record.error.is_none());
    }
}
