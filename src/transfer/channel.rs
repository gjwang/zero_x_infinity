//! Trading Transfer Channel
//!
//! Cross-thread communication channel for internal transfers with UBSCore.
//! Uses oneshot channels for request-response pattern.

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error};

use crate::transfer::types::InternalTransferId;

// ============================================================================
// Transfer Request/Response Types
// ============================================================================

/// Transfer operation type
#[derive(Debug, Clone, Copy)]
pub enum TransferOp {
    Withdraw,
    Deposit,
}

/// Transfer request to UBSCore
#[derive(Debug)]
pub struct TransferRequest {
    pub transfer_id: InternalTransferId,
    pub op: TransferOp,
    pub user_id: u64,
    pub asset_id: u32,
    pub amount: u64,
    /// Response channel (oneshot)
    pub response_tx: oneshot::Sender<TransferResponse>,
}

/// Transfer response from UBSCore
#[derive(Debug, Clone)]
pub enum TransferResponse {
    Success { avail: u64, frozen: u64 },
    Failed(String),
}

// ============================================================================
// Transfer Channel
// ============================================================================

/// Sender side of transfer channel (used by TradingAdapter)
#[derive(Clone)]
pub struct TransferSender {
    tx: mpsc::Sender<TransferRequest>,
}

impl TransferSender {
    /// Send transfer request and wait for response
    pub async fn send_request(
        &self,
        transfer_id: InternalTransferId,
        op: TransferOp,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> Result<TransferResponse, String> {
        let (response_tx, response_rx) = oneshot::channel();

        let request = TransferRequest {
            transfer_id,
            op,
            user_id,
            asset_id,
            amount,
            response_tx,
        };

        self.tx
            .send(request)
            .await
            .map_err(|_| "Transfer channel closed".to_string())?;

        response_rx
            .await
            .map_err(|_| "Transfer response channel closed".to_string())
    }
}

/// Receiver side of transfer channel (used by UBSCore thread)
pub struct TransferReceiver {
    rx: mpsc::Receiver<TransferRequest>,
}

impl TransferReceiver {
    /// Try to receive a transfer request (non-blocking)
    pub fn try_recv(&mut self) -> Option<TransferRequest> {
        self.rx.try_recv().ok()
    }

    /// Receive a transfer request (blocking until available or closed)
    pub async fn recv(&mut self) -> Option<TransferRequest> {
        self.rx.recv().await
    }
}

/// Create a new transfer channel pair
pub fn transfer_channel(buffer: usize) -> (TransferSender, TransferReceiver) {
    let (tx, rx) = mpsc::channel(buffer);
    (TransferSender { tx }, TransferReceiver { rx })
}

// ============================================================================
// UBSCore Transfer Handler
// ============================================================================

use crate::UBSCore;
use std::collections::HashSet;

/// Process pending transfer requests on UBSCore thread
///
/// Call this from the UBSCore thread's main loop to handle incoming transfers.
/// Returns the number of transfers processed.
pub fn process_transfer_requests(
    ubscore: &mut UBSCore,
    receiver: &mut TransferReceiver,
    processed_set: &mut HashSet<InternalTransferId>,
    max_per_batch: usize,
) -> usize {
    let mut count = 0;

    while count < max_per_batch {
        match receiver.try_recv() {
            Some(req) => {
                let response = process_single_transfer(ubscore, &req, processed_set);
                // Send response (ignore errors if receiver dropped)
                let _ = req.response_tx.send(response);
                count += 1;
            }
            None => break,
        }
    }

    count
}

fn process_single_transfer(
    ubscore: &mut UBSCore,
    req: &TransferRequest,
    processed_set: &mut HashSet<InternalTransferId>,
) -> TransferResponse {
    // Idempotency check
    if processed_set.contains(&req.transfer_id) {
        debug!(transfer_id = %req.transfer_id, op = ?req.op, "Transfer already processed");
        // Return success for idempotent replay
        // TODO: Should return actual balance, but we don't track it
        return TransferResponse::Success {
            avail: 0,
            frozen: 0,
        };
    }

    let result = match req.op {
        TransferOp::Withdraw => {
            ubscore.withdraw_for_transfer(req.user_id, req.asset_id, req.amount)
        }
        TransferOp::Deposit => ubscore.deposit_from_transfer(req.user_id, req.asset_id, req.amount),
    };

    match result {
        Ok((avail, frozen)) => {
            processed_set.insert(req.transfer_id);
            debug!(
                transfer_id = %req.transfer_id,
                op = ?req.op,
                avail = avail,
                frozen = frozen,
                "Transfer processed"
            );
            TransferResponse::Success { avail, frozen }
        }
        Err(e) => {
            error!(transfer_id = %req.transfer_id, op = ?req.op, error = e, "Transfer failed");
            TransferResponse::Failed(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transfer_channel_send_receive() {
        let (sender, mut receiver) = transfer_channel(10);
        let test_transfer_id = crate::transfer::InternalTransferId::new();

        // Spawn sender task
        let sender_task = tokio::spawn({
            let transfer_id = test_transfer_id;
            async move {
                sender
                    .send_request(transfer_id, TransferOp::Deposit, 1, 1, 1000)
                    .await
            }
        });

        // Receive and respond
        let req = receiver.recv().await.unwrap();
        assert_eq!(req.transfer_id, test_transfer_id);
        assert_eq!(req.user_id, 1);
        assert_eq!(req.amount, 1000);

        req.response_tx
            .send(TransferResponse::Success {
                avail: 1000,
                frozen: 0,
            })
            .unwrap();

        // Check sender's response
        let response = sender_task.await.unwrap().unwrap();
        assert!(matches!(
            response,
            TransferResponse::Success { avail: 1000, .. }
        ));
    }
}
