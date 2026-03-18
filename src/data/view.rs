use crate::common::types::InstrumentId;
use crate::data::models::{Bar, Instrument, MarketData};

pub trait MarketDataView: Send + Sync + std::fmt::Debug {
    fn get_bar_at(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar>;
    fn get_bar_at_or_before(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar>;
    fn bars_for_instrument_range(
        &self,
        instrument_id: InstrumentId,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Vec<Bar>;
    fn market_timeline(&self) -> Vec<i64>;
    fn get_id(&self, symbol: &str) -> Option<InstrumentId>;
    fn get_symbol(&self, id: InstrumentId) -> Option<String>;
    fn get_instrument(&self, id: InstrumentId) -> Option<Instrument>;
    fn iter_ids(&self) -> Vec<(InstrumentId, String)>;
    fn instrument_count(&self) -> usize;
    fn min_instrument_id(&self) -> Option<InstrumentId>;
}

impl MarketDataView for MarketData {
    fn get_bar_at(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar> {
        self.get_bar_at(instrument_id, timestamp).copied()
    }

    fn get_bar_at_or_before(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar> {
        self.get_bar_at_or_before(instrument_id, timestamp).copied()
    }

    fn bars_for_instrument_range(
        &self,
        instrument_id: InstrumentId,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Vec<Bar> {
        let Some(bars) = self.bars.get(&instrument_id) else {
            return Vec::new();
        };

        let start_index = bars.partition_point(|bar| bar.timestamp < start_timestamp);
        let end_index = bars.partition_point(|bar| bar.timestamp <= end_timestamp);

        bars[start_index..end_index].to_vec()
    }

    fn market_timeline(&self) -> Vec<i64> {
        self.market_timeline().collect()
    }

    fn get_id(&self, symbol: &str) -> Option<InstrumentId> {
        self.get_id(symbol)
    }

    fn get_symbol(&self, id: InstrumentId) -> Option<String> {
        self.get_symbol(id).map(|value| value.to_string())
    }

    fn get_instrument(&self, id: InstrumentId) -> Option<Instrument> {
        self.get_instrument(id).cloned()
    }

    fn iter_ids(&self) -> Vec<(InstrumentId, String)> {
        self.ids
            .iter()
            .map(|(instrument_id, symbol)| (*instrument_id, symbol.clone()))
            .collect()
    }

    fn instrument_count(&self) -> usize {
        self.instruments.len()
    }

    fn min_instrument_id(&self) -> Option<InstrumentId> {
        self.bars.keys().copied().min()
    }
}
