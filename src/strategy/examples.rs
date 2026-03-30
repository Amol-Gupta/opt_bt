pub mod atm_straddle_sell;
pub mod nifty_nearest_expiry_straddle;
pub mod random;
pub mod sma_nifty50;

pub use atm_straddle_sell::AtmStraddleSellStrategy;
pub use nifty_nearest_expiry_straddle::NiftyNearestExpiryStraddleStrategy;
pub use random::RandomStrategy;
pub use sma_nifty50::SmaNifty50Strategy;
