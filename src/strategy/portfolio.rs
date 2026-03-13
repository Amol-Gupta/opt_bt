use crate::common::context::Context;
use crate::common::event::{FillEvent, MarketEvent, OrderEvent, SignalEvent};
use crate::strategy::Strategy;
use std::collections::HashMap;

pub struct PortfolioStrategy {
    strategies: HashMap<String, Box<dyn Strategy + Send>>,
    // We need to map OrderID -> StrategyID to route fills/updates
    order_map: HashMap<u64, String>,
}

impl PortfolioStrategy {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
            order_map: HashMap::new(),
        }
    }

    pub fn add_strategy(&mut self, id: &str, strategy: Box<dyn Strategy + Send>) {
        self.strategies.insert(id.to_string(), strategy);
    }
}

impl Strategy for PortfolioStrategy {
    fn init(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.init(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn before_open(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.before_open(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn after_close(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.after_close(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn on_start(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.on_start(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn on_stop(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.on_stop(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn on_date_change(&mut self, ctx: &mut Context) {
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.on_date_change(ctx);
        }
        ctx.set_strategy_id("default");
    }

    fn on_market_event(&mut self, ctx: &mut Context, event: &MarketEvent) {
        // Broadcast market event to ALL strategies
        // Alternatively, strategies could subscribe to instruments.
        // For MVP, broadcast all.
        for (id, strategy) in &mut self.strategies {
            ctx.set_strategy_id(id);
            strategy.on_market_event(ctx, event);
        }
        ctx.set_strategy_id("default");
    }

    fn on_signal(&mut self, ctx: &mut Context, event: &SignalEvent) {
        if let Some(strategy) = self.strategies.get_mut(&event.strategy_id) {
            ctx.set_strategy_id(&event.strategy_id);
            strategy.on_signal(ctx, event);
        } else {
            ctx.warn(format!(
                "unroutable signal event: strategy_id='{}' ts={} instrument_id={}",
                event.strategy_id, event.timestamp, event.instrument_id
            ));
        }
        ctx.set_strategy_id("default");
    }

    fn on_order_event(&mut self, ctx: &mut Context, event: &OrderEvent) {
        // Record order -> strategy mapping to support deterministic fill fallback routing.
        self.order_map
            .insert(event.order_id, event.strategy_id.clone());

        if let Some(strategy) = self.strategies.get_mut(&event.strategy_id) {
            ctx.set_strategy_id(&event.strategy_id);
            strategy.on_order_event(ctx, event);
        } else {
            ctx.warn(format!(
                "unroutable order event: strategy_id='{}' order_id={} ts={}",
                event.strategy_id, event.order_id, event.timestamp
            ));
        }
        ctx.set_strategy_id("default");
    }

    fn on_fill(&mut self, ctx: &mut Context, event: &FillEvent) {
        if let Some(strategy) = self.strategies.get_mut(&event.strategy_id) {
            ctx.set_strategy_id(&event.strategy_id);
            strategy.on_fill(ctx, event);
            ctx.set_strategy_id("default");
            return;
        }

        if let Some(mapped_id) = self.order_map.get(&event.order_id).cloned() {
            if let Some(strategy) = self.strategies.get_mut(&mapped_id) {
                ctx.set_strategy_id(&mapped_id);
                strategy.on_fill(ctx, event);
                ctx.set_strategy_id("default");
                return;
            }

            ctx.warn(format!(
                "unroutable fill event: missing mapped strategy_id='{}' for order_id={} event_strategy_id='{}' ts={}",
                mapped_id, event.order_id, event.strategy_id, event.timestamp
            ));
        } else {
            ctx.warn(format!(
                "unroutable fill event: no route for order_id={} event_strategy_id='{}' ts={}",
                event.order_id, event.strategy_id, event.timestamp
            ));
        }

        ctx.set_strategy_id("default");
    }
}
