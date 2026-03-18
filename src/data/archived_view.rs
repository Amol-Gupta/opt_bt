use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use memmap2::MmapOptions;
use rkyv::rend::{i64_le, u32_le};

use crate::common::types::InstrumentId;
use crate::data::models::{ArchivedBar, ArchivedMarketData, Bar, Instrument};
use crate::data::view::MarketDataView;

#[derive(Debug)]
pub struct ArchivedMarketDataView {
    mmap: Arc<memmap2::Mmap>,
}

impl ArchivedMarketDataView {
    pub fn from_path(path: &Path) -> Result<Self> {
        Self::from_path_internal(path, true)
    }

    pub fn from_path_trusted(path: &Path) -> Result<Self> {
        Self::from_path_internal(path, false)
    }

    fn from_path_internal(path: &Path, validate_archive: bool) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open snapshot file {}", path.display()))?;
        let mmap = unsafe { MmapOptions::new().map(&file) }
            .with_context(|| format!("failed to mmap snapshot file {}", path.display()))?;

        if validate_archive {
            let bytes = &mmap[..];
            let _ = rkyv::api::low::access::<ArchivedMarketData, rkyv::rancor::Error>(bytes)
                .with_context(|| {
                    format!("failed to validate archived snapshot {}", path.display())
                })?;
        }

        Ok(Self {
            mmap: Arc::new(mmap),
        })
    }

    fn archived(&self) -> &ArchivedMarketData {
        unsafe { rkyv::access_unchecked::<ArchivedMarketData>(&self.mmap[..]) }
    }

    fn decode_bar(value: &ArchivedBar) -> Option<Bar> {
        rkyv::deserialize::<Bar, rkyv::rancor::Error>(value).ok()
    }

    fn decode_instrument(value: &crate::data::models::ArchivedInstrument) -> Option<Instrument> {
        rkyv::deserialize::<Instrument, rkyv::rancor::Error>(value).ok()
    }
}

impl MarketDataView for ArchivedMarketDataView {
    fn get_bar_at(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar> {
        let archived_ts = i64_le::from_native(timestamp);
        let archived_id = u32_le::from_native(instrument_id);
        let at_time = self.archived().bars_by_time.get(&archived_ts)?;
        let bar = at_time.get(&archived_id)?;
        Self::decode_bar(bar)
    }

    fn get_bar_at_or_before(&self, instrument_id: InstrumentId, timestamp: i64) -> Option<Bar> {
        if let Some(bar) = self.get_bar_at(instrument_id, timestamp) {
            return Some(bar);
        }

        let archived_id = u32_le::from_native(instrument_id);
        let bars = self.archived().bars.get(&archived_id)?;
        let mut low = 0usize;
        let mut high = bars.len();

        while low < high {
            let mid = low + (high - low) / 2;
            let candidate = bars.get(mid)?;
            let candidate_ts = i64::from(candidate.timestamp);

            if candidate_ts <= timestamp {
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        if low == 0 {
            None
        } else {
            bars.get(low - 1).and_then(Self::decode_bar)
        }
    }

    fn bars_for_instrument_range(
        &self,
        instrument_id: InstrumentId,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Vec<Bar> {
        let archived_id = u32_le::from_native(instrument_id);
        let Some(bars) = self.archived().bars.get(&archived_id) else {
            return Vec::new();
        };

        let mut start_index = 0usize;
        let mut high = bars.len();
        while start_index < high {
            let mid = start_index + (high - start_index) / 2;
            let Some(candidate) = bars.get(mid) else {
                break;
            };
            if i64::from(candidate.timestamp) < start_timestamp {
                start_index = mid + 1;
            } else {
                high = mid;
            }
        }

        let mut end_index = start_index;
        let mut high = bars.len();
        while end_index < high {
            let mid = end_index + (high - end_index) / 2;
            let Some(candidate) = bars.get(mid) else {
                break;
            };
            if i64::from(candidate.timestamp) <= end_timestamp {
                end_index = mid + 1;
            } else {
                high = mid;
            }
        }

        (start_index..end_index)
            .filter_map(|index| bars.get(index).and_then(Self::decode_bar))
            .collect()
    }

    fn market_timeline(&self) -> Vec<i64> {
        self.archived()
            .bars_by_time
            .iter()
            .map(|(timestamp, _)| i64::from(*timestamp))
            .collect()
    }

    fn get_id(&self, symbol: &str) -> Option<InstrumentId> {
        self.archived()
            .instruments
            .get(symbol)
            .map(|value| u32::from(*value))
    }

    fn get_symbol(&self, id: InstrumentId) -> Option<String> {
        let archived_id = u32_le::from_native(id);
        self.archived()
            .ids
            .get(&archived_id)
            .map(|value| value.as_str().to_string())
    }

    fn get_instrument(&self, id: InstrumentId) -> Option<Instrument> {
        let archived_id = u32_le::from_native(id);
        let archived = self.archived().instrument_meta.get(&archived_id)?;
        Self::decode_instrument(archived)
    }

    fn iter_ids(&self) -> Vec<(InstrumentId, String)> {
        self.archived()
            .ids
            .iter()
            .map(|(instrument_id, symbol)| (u32::from(*instrument_id), symbol.as_str().to_string()))
            .collect()
    }

    fn instrument_count(&self) -> usize {
        self.archived().instruments.len()
    }

    fn min_instrument_id(&self) -> Option<InstrumentId> {
        self.archived()
            .bars
            .iter()
            .map(|(instrument_id, _)| u32::from(*instrument_id))
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::{InstrumentKind, OptionType, PRICE_SCALE};
    use crate::data::models::{MarketData, OptionSpec};

    fn fixture_market_data() -> MarketData {
        let mut md = MarketData::new();

        md.add_bar(
            "NIFTY 50",
            Bar {
                timestamp: 1_704_067_200,
                open: 21_500 * PRICE_SCALE,
                high: 21_520 * PRICE_SCALE,
                low: 21_490 * PRICE_SCALE,
                close: 21_510 * PRICE_SCALE,
                volume: 100,
            },
        );
        md.add_bar(
            "NIFTY 50",
            Bar {
                timestamp: 1_704_067_260,
                open: 21_510 * PRICE_SCALE,
                high: 21_530 * PRICE_SCALE,
                low: 21_505 * PRICE_SCALE,
                close: 21_525 * PRICE_SCALE,
                volume: 120,
            },
        );

        md.add_bar(
            "NIFTY11JAN2421500CE",
            Bar {
                timestamp: 1_704_067_200,
                open: 100 * PRICE_SCALE,
                high: 106 * PRICE_SCALE,
                low: 98 * PRICE_SCALE,
                close: 105 * PRICE_SCALE,
                volume: 60,
            },
        );
        md.add_bar(
            "NIFTY11JAN2421500CE",
            Bar {
                timestamp: 1_704_067_260,
                open: 105 * PRICE_SCALE,
                high: 108 * PRICE_SCALE,
                low: 102 * PRICE_SCALE,
                close: 103 * PRICE_SCALE,
                volume: 55,
            },
        );

        md
    }

    fn fixture_snapshot_path() -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "opt_bt_archived_view_parity_{}_{}.rkyv",
            std::process::id(),
            nanos
        ))
    }

    fn build_views() -> (MarketData, ArchivedMarketDataView, std::path::PathBuf) {
        let md = fixture_market_data();
        let path = fixture_snapshot_path();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&md).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let archived = ArchivedMarketDataView::from_path(&path).unwrap();
        (md, archived, path)
    }

    #[test]
    fn archived_view_matches_owned_for_ids_and_timeline() {
        let (owned_md, archived, path) = build_views();

        let owned: &dyn MarketDataView = &owned_md;
        let archived_view: &dyn MarketDataView = &archived;

        assert_eq!(owned.instrument_count(), archived_view.instrument_count());
        assert_eq!(owned.min_instrument_id(), archived_view.min_instrument_id());
        assert_eq!(owned.market_timeline(), archived_view.market_timeline());

        for (instrument_id, symbol) in owned.iter_ids() {
            assert_eq!(
                owned.get_symbol(instrument_id),
                archived_view.get_symbol(instrument_id)
            );
            assert_eq!(owned.get_id(&symbol), archived_view.get_id(&symbol));
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn archived_view_matches_owned_for_bar_access_and_instruments() {
        let (owned_md, archived, path) = build_views();

        let owned: &dyn MarketDataView = &owned_md;
        let archived_view: &dyn MarketDataView = &archived;

        let index_id = owned.get_id("NIFTY 50").unwrap();
        let option_id = owned.get_id("NIFTY11JAN2421500CE").unwrap();

        assert_eq!(
            owned.get_bar_at(index_id, 1_704_067_200),
            archived_view.get_bar_at(index_id, 1_704_067_200)
        );
        assert_eq!(
            owned.get_bar_at(option_id, 1_704_067_260),
            archived_view.get_bar_at(option_id, 1_704_067_260)
        );

        assert_eq!(
            owned.get_bar_at_or_before(index_id, 1_704_067_245),
            archived_view.get_bar_at_or_before(index_id, 1_704_067_245)
        );
        assert_eq!(
            owned.get_bar_at_or_before(option_id, 1_704_067_280),
            archived_view.get_bar_at_or_before(option_id, 1_704_067_280)
        );

        assert_eq!(
            owned.get_instrument(index_id),
            archived_view.get_instrument(index_id)
        );
        assert_eq!(
            owned.get_instrument(option_id),
            archived_view.get_instrument(option_id)
        );

        let option = archived_view.get_instrument(option_id).unwrap();
        assert_eq!(option.kind, InstrumentKind::Option);
        let option_spec = option.option.unwrap_or(OptionSpec {
            underlying: String::new(),
            expiry_yyyymmdd: 0,
            strike: 0,
            option_type: OptionType::Call,
        });
        assert_eq!(option_spec.underlying, "NIFTY");
        assert_eq!(option_spec.expiry_yyyymmdd, 20240111);

        let _ = std::fs::remove_file(path);
    }
}
