macro_rules! impl_display_via_debug {
    ($($t:path),+ $(,)?) => {
        $(
            impl std::fmt::Display for $t {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, "{:?}", self)
                }
            }
        )+
    };
}

macro_rules! impl_display_via_name {
    ($($t:ty),+ $(,)?) => {
        $(
            impl std::fmt::Display for $t {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write!(f, stringify!($t))
                }
            }
        )+
    };
}

impl_display_via_debug!(
    crate::config::Config,
    crate::config::PortfolioConfig,
    crate::config::StrategyConfig,
    crate::config::OptionFilterConfig,
    crate::config::SweepConfig,
    crate::common::types::OptionType,
    crate::common::types::InstrumentKind,
    crate::common::types::Side,
    crate::common::types::OrderType,
    crate::common::types::Status,
    crate::common::types::TimeInForce,
    crate::common::context::SubscriptionDiff,
    crate::common::context::Context,
    crate::common::event::Event,
    crate::common::event::MarketEvent,
    crate::common::event::SignalEvent,
    crate::common::event::OrderEvent,
    crate::common::event::FillEvent,
    crate::common::event::EventQueue,
    crate::reporting::post_analysis::TaxAdjustedTrade,
    crate::reporting::post_analysis::TaxSummary,
    crate::reporting::post_analysis::StressScenario,
    crate::reporting::post_analysis::StressResult,
    crate::reporting::post_analysis::PostAnalysisSummary,
    crate::reporting::post_analysis::FlatRateTaxModel,
    crate::reporting::metrics::MetricValues,
    crate::reporting::portfolio::PortfolioView,
    crate::reporting::portfolio::StrategyPortfolioBreakdown,
    crate::reporting::json::BacktestReport,
    crate::reporting::json::Reproducibility,
    crate::reporting::json::DatasetMetadata,
    crate::reporting::json::Simulation,
    crate::reporting::json::Metrics,
    crate::reporting::json::FillRecord,
    crate::reporting::json::OrderEventRecord,
    crate::reporting::json::PositionEventRecord,
    crate::reporting::json::StrategyAttributionRecord,
    crate::reporting::json::EquityPoint,
    crate::reporting::aggregator::AggregatedSummary,
    crate::engine::sweep::SweepResult,
    crate::data::models::OptionSpec,
    crate::data::models::Instrument,
    crate::data::models::Bar,
    crate::data::models::MarketData,
    crate::portfolio::manager::StrategyAttribution,
    crate::portfolio::manager::Account,
    crate::portfolio::models::Order,
    crate::portfolio::models::Trade,
    crate::portfolio::models::Position,
    crate::portfolio::allocator::AllocationRule,
    crate::portfolio::allocator::PortfolioAllocator,
    crate::execution::slippage::NoSlippage,
    crate::execution::slippage::FixedSlippage,
    crate::execution::stale::DataStatus,
    crate::execution::stale::StaleDetector,
    crate::strategy::OptionFilterCriteria,
);

impl<S: crate::strategy::Strategy> std::fmt::Display for crate::engine::runner::Engine<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Engine")
    }
}

impl_display_via_name!(
    crate::common::logging::LoggerGuard,
    crate::data::loader::DataLoader,
    crate::execution::fill::DefaultFillModel,
    crate::strategy::portfolio::PortfolioStrategy,
    crate::strategy::examples::SmaNifty50Strategy,
    crate::strategy::examples::NiftyNearestExpiryStraddleStrategy,
    crate::strategy::examples::RandomStrategy,
    crate::strategy::examples::AtmStraddleSellStrategy,
);
