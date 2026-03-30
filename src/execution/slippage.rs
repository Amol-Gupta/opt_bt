use crate::common::types::{Price, Side};

pub trait SlippageModel {
    fn calculate_slippage(&self, price: Price, quantity: i64, side: Side) -> Price;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoSlippage;

impl SlippageModel for NoSlippage {
    fn calculate_slippage(&self, price: Price, _quantity: i64, _side: Side) -> Price {
        price
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedSlippage {
    pub ticks: i64,
    pub tick_size: i64,
}

impl FixedSlippage {
    pub fn new(ticks: i64, tick_size: i64) -> Self {
        Self { ticks, tick_size }
    }
}

impl SlippageModel for FixedSlippage {
    fn calculate_slippage(&self, price: Price, _quantity: i64, side: Side) -> Price {
        let slippage = self.ticks * self.tick_size;
        match side {
            Side::Buy => price + slippage,
            Side::Sell => price - slippage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_slippage() {
        let model = FixedSlippage::new(2, 5); // 2 ticks * 5 = 10 units
        let price = 1000;

        assert_eq!(model.calculate_slippage(price, 1, Side::Buy), 1010);
        assert_eq!(model.calculate_slippage(price, 1, Side::Sell), 990);
    }
}
