use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataStatus {
    Fresh,
    Stale,
    Future, // Data is in the future somehow
    NoData,
}

#[derive(Debug, Clone, Copy)]
pub struct StaleDetector {
    pub threshold: Duration,
}

impl StaleDetector {
    pub fn new(threshold_seconds: u64) -> Self {
        Self {
            threshold: Duration::from_secs(threshold_seconds),
        }
    }

    pub fn check(&self, current_ts: i64, data_ts: i64) -> DataStatus {
        if data_ts > current_ts {
            return DataStatus::Future;
        }
        
        // Assuming timestamps are seconds
        // Be careful with i64 subtraction overflow (unlikely for unix ts)
        let diff = current_ts - data_ts;
        
        if diff < 0 {
             return DataStatus::Future; // Should be covered above
        }
        
        if diff as u64 > self.threshold.as_secs() {
            return DataStatus::Stale;
        }
        
        DataStatus::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stale_detection() {
        let detector = StaleDetector::new(60); // 1 minute threshold
        let now = 1000;

        assert_eq!(detector.check(now, 1000), DataStatus::Fresh);
        assert_eq!(detector.check(now, 940), DataStatus::Fresh); // 60s diff
        assert_eq!(detector.check(now, 939), DataStatus::Stale); // 61s diff
        assert_eq!(detector.check(now, 1001), DataStatus::Future);
    }
}
