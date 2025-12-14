#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub price: f64, // CRIME: Using f64 for financial data
    pub qty: f64,
    pub side: Side,
}

impl Order {
    pub fn new(id: u64, price: f64, qty: f64, side: Side) -> Self {
        Self { id, price, qty, side }
    }
}