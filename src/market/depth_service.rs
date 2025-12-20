// Market depth service
//
// Consumes DepthSnapshot from ME and serves HTTP queries

use crate::messages::DepthSnapshot;
use crate::pipeline::MultiThreadQueues;
use std::sync::Arc;
use std::sync::RwLock;

/// DepthService - consumes depth snapshots and serves queries
pub struct DepthService {
    /// Current depth snapshot
    current_snapshot: Arc<RwLock<DepthSnapshot>>,
    /// Queue to consume snapshots from
    queues: Arc<MultiThreadQueues>,
}

impl DepthService {
    pub fn new(queues: Arc<MultiThreadQueues>) -> Self {
        Self {
            current_snapshot: Arc::new(RwLock::new(DepthSnapshot::empty())),
            queues,
        }
    }

    /// Run the service - consume snapshots from queue
    pub async fn run(&self) {
        let mut spin_count = 0u32;
        const IDLE_SPIN_LIMIT: u32 = 1000;

        loop {
            // Try to consume snapshot from queue
            if let Some(snapshot) = self.queues.depth_event_queue.pop() {
                // Update current snapshot
                if let Ok(mut current) = self.current_snapshot.write() {
                    *current = snapshot;
                }
                spin_count = 0;
            } else {
                // No snapshot available, spin or yield
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            }
        }
    }

    /// Get current snapshot for HTTP queries
    pub fn get_snapshot(&self, limit: usize) -> DepthSnapshot {
        let snapshot = self.current_snapshot.read().unwrap();

        // Limit the number of levels returned
        let bids: Vec<(u64, u64)> = snapshot.bids.iter().take(limit).copied().collect();
        let asks: Vec<(u64, u64)> = snapshot.asks.iter().take(limit).copied().collect();

        DepthSnapshot::new(bids, asks, snapshot.update_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_service_get_snapshot() {
        let queues = Arc::new(MultiThreadQueues::new());
        let service = DepthService::new(queues.clone());

        // Initially empty
        let snapshot = service.get_snapshot(10);
        assert_eq!(snapshot.bids.len(), 0);
        assert_eq!(snapshot.asks.len(), 0);

        // Push a snapshot
        let test_snapshot = DepthSnapshot::new(
            vec![(30000, 100), (29900, 200), (29800, 300)],
            vec![(30100, 150), (30200, 250)],
            42,
        );
        queues.depth_event_queue.push(test_snapshot).unwrap();

        // Manually update (simulating what run() does)
        if let Some(snap) = queues.depth_event_queue.pop() {
            *service.current_snapshot.write().unwrap() = snap;
        }

        // Now should have data
        let snapshot = service.get_snapshot(2);
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);
        assert_eq!(snapshot.update_id, 42);
        assert_eq!(snapshot.bids[0], (30000, 100));
    }
}
