//! Transfer FSM State Definitions
//!
//! State IDs match the design document for PostgreSQL storage.

use std::fmt;

/// Transfer FSM States
///
/// State IDs are designed for PostgreSQL storage as SMALLINT.
/// Terminal states: COMMITTED (40), FAILED (-10), ROLLED_BACK (-30)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i16)]
pub enum TransferState {
    /// Initial state - request validated and recorded
    Init = 0,

    /// Source withdrawal initiated (persist-before-call)
    SourcePending = 10,

    /// Source withdrawal confirmed - funds are IN-FLIGHT
    /// CRITICAL: Must eventually reach COMMITTED or ROLLED_BACK
    SourceDone = 20,

    /// Target deposit initiated (persist-before-call)
    TargetPending = 30,

    /// Terminal: Transfer completed successfully
    Committed = 40,

    /// Terminal: Source withdrawal failed (no funds moved)
    Failed = -10,

    /// Compensation in progress (refunding source)
    Compensating = -20,

    /// Terminal: Source refund completed
    RolledBack = -30,
}

impl TransferState {
    /// Check if this is a terminal state (no more transitions possible)
    #[inline]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransferState::Committed | TransferState::Failed | TransferState::RolledBack
        )
    }

    /// Check if funds are in-flight (source deducted, target not confirmed)
    #[inline]
    pub fn is_in_flight(&self) -> bool {
        matches!(
            self,
            TransferState::SourceDone | TransferState::TargetPending | TransferState::Compensating
        )
    }

    /// Get the numeric state ID for PostgreSQL storage
    #[inline]
    pub fn id(&self) -> i16 {
        *self as i16
    }

    /// Convert from PostgreSQL state ID
    pub fn from_id(id: i16) -> Option<Self> {
        match id {
            0 => Some(TransferState::Init),
            10 => Some(TransferState::SourcePending),
            20 => Some(TransferState::SourceDone),
            30 => Some(TransferState::TargetPending),
            40 => Some(TransferState::Committed),
            -10 => Some(TransferState::Failed),
            -20 => Some(TransferState::Compensating),
            -30 => Some(TransferState::RolledBack),
            _ => None,
        }
    }

    /// Get human-readable state name
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferState::Init => "INIT",
            TransferState::SourcePending => "SOURCE_PENDING",
            TransferState::SourceDone => "SOURCE_DONE",
            TransferState::TargetPending => "TARGET_PENDING",
            TransferState::Committed => "COMMITTED",
            TransferState::Failed => "FAILED",
            TransferState::Compensating => "COMPENSATING",
            TransferState::RolledBack => "ROLLED_BACK",
        }
    }
}

impl fmt::Display for TransferState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<i16> for TransferState {
    type Error = ();

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        TransferState::from_id(value).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_states() {
        assert!(TransferState::Committed.is_terminal());
        assert!(TransferState::Failed.is_terminal());
        assert!(TransferState::RolledBack.is_terminal());

        assert!(!TransferState::Init.is_terminal());
        assert!(!TransferState::SourcePending.is_terminal());
        assert!(!TransferState::SourceDone.is_terminal());
        assert!(!TransferState::TargetPending.is_terminal());
        assert!(!TransferState::Compensating.is_terminal());
    }

    #[test]
    fn test_in_flight_states() {
        assert!(TransferState::SourceDone.is_in_flight());
        assert!(TransferState::TargetPending.is_in_flight());
        assert!(TransferState::Compensating.is_in_flight());

        assert!(!TransferState::Init.is_in_flight());
        assert!(!TransferState::SourcePending.is_in_flight());
        assert!(!TransferState::Committed.is_in_flight());
        assert!(!TransferState::Failed.is_in_flight());
        assert!(!TransferState::RolledBack.is_in_flight());
    }

    #[test]
    fn test_state_id_roundtrip() {
        let states = [
            TransferState::Init,
            TransferState::SourcePending,
            TransferState::SourceDone,
            TransferState::TargetPending,
            TransferState::Committed,
            TransferState::Failed,
            TransferState::Compensating,
            TransferState::RolledBack,
        ];

        for state in states {
            let id = state.id();
            let recovered = TransferState::from_id(id).unwrap();
            assert_eq!(state, recovered);
        }
    }

    #[test]
    fn test_invalid_state_id() {
        assert!(TransferState::from_id(999).is_none());
        assert!(TransferState::from_id(-999).is_none());
    }

    #[test]
    fn test_display() {
        assert_eq!(TransferState::Init.to_string(), "INIT");
        assert_eq!(TransferState::Committed.to_string(), "COMMITTED");
        assert_eq!(TransferState::RolledBack.to_string(), "ROLLED_BACK");
    }
}
