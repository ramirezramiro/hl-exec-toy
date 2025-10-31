
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side { Buy, Sell }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub side: Side,
    pub price: i64,   // integer ticks
    pub qty: i64,     // integer qty
    pub ts: u64,      // lamport/logical or ns
}

#[derive(Debug, Default)]
pub struct Book {
    // simple price-time FIFO; for brevity just keep vectors
    bids: Vec<Order>,
    asks: Vec<Order>,
}

impl Book {
    pub fn new() -> Self { Self::default() }

    pub fn submit(&mut self, mut o: Order) -> Vec<(u64, i64, i64)> {
        // returns trades: (taker_id, maker_id, qty)
        let mut trades = Vec::new();
        match o.side {
            Side::Buy => {
                // match against asks: best price lowest first
                self.asks.sort_by_key(|x| (x.price, x.ts));
                let mut i = 0;
                while i < self.asks.len() && o.qty > 0 && o.price >= self.asks[i].price {
                    let maker = self.asks[i];
                    let fill = o.qty.min(maker.qty);
                    trades.push((o.id, maker.id, fill));
                    o.qty -= fill;
                    if fill == maker.qty { self.asks.remove(i); } else { self.asks[i].qty -= fill; i += 1; }
                }
                if o.qty > 0 { self.bids.push(o); }
            }
            Side::Sell => {
                // match against bids: best price highest first
                self.bids.sort_by_key(|x| (-x.price, x.ts));
                let mut i = 0;
                while i < self.bids.len() && o.qty > 0 && o.price <= self.bids[i].price {
                    let maker = self.bids[i];
                    let fill = o.qty.min(maker.qty);
                    trades.push((o.id, maker.id, fill));
                    o.qty -= fill;
                    if fill == maker.qty { self.bids.remove(i); } else { self.bids[i].qty -= fill; i += 1; }
                }
                if o.qty > 0 { self.asks.push(o); }
            }
        }
        trades
    }

    pub fn depth(&self) -> (Vec<(i64, i64)>, Vec<(i64, i64)>) {
        // naive depth aggregation by price
        let mut bids_map = std::collections::BTreeMap::new();
        let mut asks_map = std::collections::BTreeMap::new();
        for b in &self.bids { *bids_map.entry(b.price).or_insert(0) += b.qty; }
        for a in &self.asks { *asks_map.entry(a.price).or_insert(0) += a.qty; }
        let bids = bids_map.iter().rev().map(|(p,q)| (*p,*q)).collect();
        let asks = asks_map.iter().map(|(p,q)| (*p,*q)).collect();
        (bids, asks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn simple_cross() {
        let mut b = Book::new();
        b.submit(Order { id: 1, side: Side::Buy, price: 100, qty: 10, ts: 1 });
        let trades = b.submit(Order { id: 2, side: Side::Sell, price: 100, qty: 6, ts: 2 });
        assert_eq!(trades, vec![(2,1,6)]);
        let (bids, asks) = b.depth();
        assert_eq!(bids, vec![(100, 4)]);
        assert!(asks.is_empty());
    }

    proptest! {
        // basic invariant: all trades have qty > 0 and remaining book is consistent
        #[test]
        fn prop_qty_positive(o_qty in 1i64..1000) {
            let mut b = Book::new();
            b.submit(Order { id: 1, side: Side::Buy, price: 100, qty: o_qty, ts: 1 });
            let trades = b.submit(Order { id: 2, side: Side::Sell, price: 100, qty: o_qty, ts: 2 });
            for (_,_,q) in trades { prop_assert!(q > 0); }
        }
    }
}
