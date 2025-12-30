use zero_x_infinity::engine::MatchingEngine;
use zero_x_infinity::models::{InternalOrder, Side, TimeInForce};
use zero_x_infinity::orderbook::OrderBook; // OrderBook is re-exported

/// Helper to create a Limit GTC Order
fn create_limit_gtc(id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
    // new(order_id, user_id, symbol_id, price, qty, side)
    InternalOrder::new(id, 1, 1, price, qty, side)
}

/// Helper to create a Limit IOC Order
fn create_limit_ioc(id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
    let mut order = InternalOrder::new(id, 2, 1, price, qty, side);
    order.time_in_force = TimeInForce::IOC;
    order
}

#[test]
fn qa_tc_ioc_sweeps_multiple_levels() {
    let mut book = OrderBook::new();

    // Setup:
    // Sell 10 @ 100
    // Sell 20 @ 101
    // Sell 30 @ 102
    let s1 = create_limit_gtc(1, 100, 10, Side::Sell);
    let s2 = create_limit_gtc(2, 101, 20, Side::Sell);
    let s3 = create_limit_gtc(3, 102, 30, Side::Sell);

    MatchingEngine::process_order(&mut book, s1);
    MatchingEngine::process_order(&mut book, s2);
    MatchingEngine::process_order(&mut book, s3);

    // Action: IOC Buy 40 @ 101 (Should fill 10@100, 20@101, and EXPIRE remaining 10)
    // Why expire remaining 10? Because Limit Price is 101. It cannot match @ 102.
    // And remaining 10 cannot rest because it is IOC.

    let taker = create_limit_ioc(4, 101, 40, Side::Buy);
    let res = MatchingEngine::process_order(&mut book, taker);

    // Verify Trades:
    // Should match 10 + 20 = 30 total.
    let total_traded: u64 = res.trades.iter().map(|t| t.qty).sum();
    assert_eq!(
        total_traded, 30,
        "IOC Sweep should match available liquidity up to limit price"
    );

    // Verify Order Status:
    // Should be EXPIRED because it had remaining quantity (10) that couldn't match (102 > 101)
    // and IOC cannot rest.
    // Note: status might be FILLED if it matched 100%? No, it matched 30/40.
    // Spec says: "If IOC mismatch remainder -> Expire".
    // If partial fill -> Expired?
    // Let's check model.rs: "IOC/FOK no fill" -> EXPIRED.
    // Actually, usually "Partially Filled IOC" final status is "EXPIRED" (Binance) or "PARTIALLY_FILLED_CANCELED"?
    // The implementation (engine.rs line 61) says: `order.status = OrderStatus::EXPIRED;`
    // So if fill < qty, it becomes EXPIRED.
    // Wait, if it filled *some*, is it "PARTIALLY_FILLED" or "EXPIRED"?
    // engine.rs:
    // if order.is_filled() { status = FILLED }
    // else if IOC { status = EXPIRED }
    // So yes, verification: Status should be EXPIRED (meaning "Done, remainder killed").
    // (Or maybe PARTIALLY_FILLED if the system tracks cumulative fills, but the final terminal state for the *remainder* is expiration).
    // The `res.order` structure represents the final state.
    // Let's assert EXPIRED if implementation follows standard IOC.
    // But check if `is_filled` check happens first.
    // 30 < 40, so `is_filled` is false.
    // So it enters `else if IOC`. So `status = EXPIRED`.

    // BUT! Since some trades occurred, `filled_qty` should be 30.
    assert_eq!(res.order.filled_qty, 30);
    assert!(
        matches!(
            res.order.status,
            zero_x_infinity::models::OrderStatus::EXPIRED
        ),
        "Partially filled IOC should end as EXPIRED (remainder killed)"
    );

    // Verify Book:
    // 102 sell should remain.
    assert_eq!(book.best_ask(), Some(102));
    // 100 and 101 sells should be gone.
}

#[test]
fn qa_tc_ioc_never_rests_sanity_check() {
    let mut book = OrderBook::new();

    // Action: IOC Buy 100 @ 100 into empty book
    let taker = create_limit_ioc(1, 100, 100, Side::Buy);
    let res = MatchingEngine::process_order(&mut book, taker);

    assert_eq!(res.trades.len(), 0);
    assert_eq!(book.all_orders().len(), 0, "IOC must not rest in book");
}

#[test]
fn qa_tc_reduce_preserves_priority_complex() {
    let mut book = OrderBook::new();

    // Setup: 3 Orders at same price
    // A: 100, B: 100, C: 100
    let a = create_limit_gtc(1, 100, 100, Side::Buy);
    let b = create_limit_gtc(2, 100, 100, Side::Buy);
    let c = create_limit_gtc(3, 100, 100, Side::Buy);

    MatchingEngine::process_order(&mut book, a);
    MatchingEngine::process_order(&mut book, b);
    MatchingEngine::process_order(&mut book, c);

    // Action: Reduce B (middle) by 50.
    MatchingEngine::reduce_order(&mut book, 2, 50).expect("Reduce should succeed");

    // Verify Book Order: A, B, C
    let orders = book.all_orders(); // This returns linear list
    // Assuming `all_orders` preserves insertion order (it usually does for test introspection)
    // Actually `OrderBook::all_orders` usually iterates over B-Tree.
    // Within same price, it should be FIFO.
    // If Reduce preserved priority, B should still be at index 1.

    assert_eq!(orders.len(), 3);
    assert_eq!(orders[0].order_id, 1, "A should be first");
    assert_eq!(orders[1].order_id, 2, "B should be second (Reduced)");
    assert_eq!(orders[1].qty, 50, "B quantity should be 50");
    assert_eq!(orders[2].order_id, 3, "C should be third");
}

#[test]
fn qa_tc_move_order_same_price_loses_priority() {
    let mut book = OrderBook::new();

    // A: 100, B: 100
    let a = create_limit_gtc(1, 100, 100, Side::Buy);
    let b = create_limit_gtc(2, 100, 100, Side::Buy);

    MatchingEngine::process_order(&mut book, a);
    MatchingEngine::process_order(&mut book, b);

    // Move A to same price (effectively "Cancel/Replace")
    MatchingEngine::move_order(&mut book, 1, 100).expect("Move should succeed");

    // Verify Order: B, A
    let orders = book.all_orders();
    assert_eq!(orders[0].order_id, 2, "B should now be first");
    assert_eq!(
        orders[1].order_id, 1,
        "A should now be last (Lost Priority)"
    );
}
