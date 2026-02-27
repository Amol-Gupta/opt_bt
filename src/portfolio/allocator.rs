use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct AllocationRule {
    pub capital_limit: i64,
    pub max_exposure_ratio: f64,
}

#[derive(Debug, Clone, Default)]
pub struct PortfolioAllocator {
    total_capital: i64,
    rules: HashMap<String, AllocationRule>,
}

impl PortfolioAllocator {
    pub fn new(total_capital: i64) -> Self {
        Self {
            total_capital,
            rules: HashMap::new(),
        }
    }

    pub fn register_strategy(&mut self, strategy_id: &str, capital_limit: i64, max_exposure_ratio: f64) {
        let ratio = max_exposure_ratio.clamp(0.0, 1.0);
        self.rules.insert(
            strategy_id.to_string(),
            AllocationRule {
                capital_limit,
                max_exposure_ratio: ratio,
            },
        );
    }

    pub fn is_within_total_budget(&self) -> bool {
        let assigned: i64 = self.rules.values().map(|rule| rule.capital_limit).sum();
        assigned <= self.total_capital
    }

    pub fn can_accept_notional(
        &self,
        strategy_id: &str,
        current_open_notional: i64,
        additional_notional: i64,
    ) -> bool {
        let Some(rule) = self.rules.get(strategy_id) else {
            return false;
        };

        if !self.is_within_total_budget() {
            return false;
        }

        let max_by_capital = rule.capital_limit;
        let max_by_exposure = ((rule.capital_limit as f64) * rule.max_exposure_ratio).round() as i64;
        let limit = max_by_capital.min(max_by_exposure.max(0));

        current_open_notional.saturating_add(additional_notional) <= limit
    }

    pub fn strategy_capital(&self, strategy_id: &str) -> Option<i64> {
        self.rules.get(strategy_id).map(|rule| rule.capital_limit)
    }
}

#[cfg(test)]
mod tests {
    use super::PortfolioAllocator;

    #[test]
    fn test_total_budget_guard() {
        let mut allocator = PortfolioAllocator::new(1_000_000);
        allocator.register_strategy("s1", 400_000, 1.0);
        allocator.register_strategy("s2", 500_000, 1.0);
        assert!(allocator.is_within_total_budget());

        allocator.register_strategy("s3", 200_000, 1.0);
        assert!(!allocator.is_within_total_budget());
    }

    #[test]
    fn test_exposure_limit_enforced() {
        let mut allocator = PortfolioAllocator::new(1_000_000);
        allocator.register_strategy("s1", 500_000, 0.5);

        assert!(allocator.can_accept_notional("s1", 100_000, 120_000));
        assert!(!allocator.can_accept_notional("s1", 180_000, 80_000));
    }

    #[test]
    fn test_unknown_strategy_rejected() {
        let allocator = PortfolioAllocator::new(1_000_000);
        assert!(!allocator.can_accept_notional("missing", 10_000, 10_000));
    }
}