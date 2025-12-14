#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub price: u64,
    pub qty: u64,
    pub side: Side,
}

impl Order {
    pub fn new(id: u64, price: u64, qty: u64, side: Side) -> Self {
        Self { id, price, qty, side }
    }
}