use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::common::types::{InstrumentId, InstrumentKind, OptionType, Price, PRICE_SCALE};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionSpec {
    pub underlying: String,
    pub expiry_yyyymmdd: i32,
    pub strike: Price,
    pub option_type: OptionType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub id: InstrumentId,
    pub symbol: String,
    pub kind: InstrumentKind,
    pub option: Option<OptionSpec>,
}

impl Instrument {
    pub fn unknown(id: InstrumentId, symbol: &str) -> Self {
        Self {
            id,
            symbol: symbol.to_string(),
            kind: InstrumentKind::Unknown,
            option: None,
        }
    }

    pub fn deterministic_key(&self) -> String {
        match &self.option {
            Some(option) => format!(
                "OPT:{}:{}:{}:{:?}",
                option.underlying, option.expiry_yyyymmdd, option.strike, option.option_type
            ),
            None => format!("{:?}:{}", self.kind, self.symbol),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub timestamp: i64, // Unix Timestamp (seconds)
    pub open: Price,    // Price * 10,000
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketData {
    // Map instrument ID to sorted bars
    pub bars: HashMap<InstrumentId, Vec<Bar>>,
    // Metadata for instruments (Symbol -> ID)
    pub instruments: HashMap<String, InstrumentId>,
    // Reverse lookup (ID -> Symbol)
    pub ids: HashMap<InstrumentId, String>,
    // Rich instrument metadata (ID -> Instrument)
    pub instrument_meta: HashMap<InstrumentId, Instrument>,
}

impl MarketData {
    pub fn new() -> Self {
        Self {
            bars: HashMap::new(),
            instruments: HashMap::new(),
            ids: HashMap::new(),
            instrument_meta: HashMap::new(),
        }
    }

    pub fn upsert_instrument(&mut self, instrument: Instrument) {
        self.instruments
            .insert(instrument.symbol.clone(), instrument.id);
        self.ids.insert(instrument.id, instrument.symbol.clone());
        self.instrument_meta.insert(instrument.id, instrument);
    }

    pub fn add_bar(&mut self, symbol: &str, bar: Bar) {
        let id = *self.instruments.entry(symbol.to_string()).or_insert_with(|| {
            let next_id = self.ids.len() as u32 + 1;
            self.ids.insert(next_id, symbol.to_string());
            self.instrument_meta
                .entry(next_id)
                .or_insert_with(|| infer_instrument(next_id, symbol));
            next_id
        });
        
        self.bars.entry(id).or_insert_with(Vec::new).push(bar);
    }
    
    pub fn get_id(&self, symbol: &str) -> Option<InstrumentId> {
        self.instruments.get(symbol).copied()
    }
    
    pub fn get_symbol(&self, id: InstrumentId) -> Option<&str> {
        self.ids.get(&id).map(|s| s.as_str())
    }

    pub fn get_instrument(&self, id: InstrumentId) -> Option<&Instrument> {
        self.instrument_meta.get(&id)
    }
}

fn infer_instrument(id: InstrumentId, symbol: &str) -> Instrument {
    if let Some(option) = parse_option_symbol(symbol) {
        return Instrument {
            id,
            symbol: symbol.to_string(),
            kind: InstrumentKind::Option,
            option: Some(option),
        };
    }

    Instrument::unknown(id, symbol)
}

fn parse_option_symbol(symbol: &str) -> Option<OptionSpec> {
    let normalized = symbol.trim().to_ascii_uppercase();
    let (base, option_type) = if let Some(value) = normalized.strip_suffix("CE") {
        (value, OptionType::Call)
    } else if let Some(value) = normalized.strip_suffix("PE") {
        (value, OptionType::Put)
    } else {
        return None;
    };

    let first_digit_index = base.find(|character: char| character.is_ascii_digit())?;
    let underlying = &base[..first_digit_index];
    let details = &base[first_digit_index..];

    if details.len() < 8 {
        return None;
    }

    let expiry_token = &details[..7];
    let strike_token = &details[7..];

    let expiry_yyyymmdd = parse_expiry_token(expiry_token)?;
    let strike_points = strike_token.parse::<i64>().ok()?;

    Some(OptionSpec {
        underlying: underlying.to_string(),
        expiry_yyyymmdd,
        strike: strike_points * PRICE_SCALE,
        option_type,
    })
}

fn parse_expiry_token(token: &str) -> Option<i32> {
    if token.len() != 7 {
        return None;
    }

    let day = token.get(0..2)?.parse::<u32>().ok()?;
    let month_token = token.get(2..5)?;
    let year_suffix = token.get(5..7)?.parse::<u32>().ok()?;

    let month = match month_token {
        "JAN" => 1,
        "FEB" => 2,
        "MAR" => 3,
        "APR" => 4,
        "MAY" => 5,
        "JUN" => 6,
        "JUL" => 7,
        "AUG" => 8,
        "SEP" => 9,
        "OCT" => 10,
        "NOV" => 11,
        "DEC" => 12,
        _ => return None,
    };

    let year = 2000 + year_suffix;
    Some((year as i32) * 10_000 + (month as i32) * 100 + day as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::PRICE_SCALE;

    #[test]
    fn test_market_data_basic() {
        let mut md = MarketData::new();
        let bar = Bar {
            timestamp: 1000,
            open: 100 * PRICE_SCALE,
            high: 105 * PRICE_SCALE,
            low: 95 * PRICE_SCALE,
            close: 102 * PRICE_SCALE,
            volume: 500,
        };
        
        md.add_bar("NIFTY", bar);
        
        assert_eq!(md.instruments.len(), 1);
        assert_eq!(md.get_id("NIFTY"), Some(1));
        assert_eq!(md.bars.get(&1).unwrap().len(), 1);
        assert_eq!(md.bars.get(&1).unwrap()[0], bar);
        assert_eq!(
            md.get_instrument(1).map(|instrument| instrument.kind),
            Some(InstrumentKind::Unknown)
        );
    }

    #[test]
    fn test_option_instrument_deterministic_key() {
        let instrument = Instrument {
            id: 10,
            symbol: "NIFTY24APR22000CE".to_string(),
            kind: InstrumentKind::Option,
            option: Some(OptionSpec {
                underlying: "NIFTY".to_string(),
                expiry_yyyymmdd: 20260430,
                strike: 22000 * PRICE_SCALE,
                option_type: OptionType::Call,
            }),
        };

        assert_eq!(
            instrument.deterministic_key(),
            format!("OPT:NIFTY:20260430:{}:Call", 22000 * PRICE_SCALE)
        );
    }

    #[test]
    fn test_add_bar_parses_option_symbol_into_instrument_metadata() {
        let mut md = MarketData::new();
        let bar = Bar {
            timestamp: 1000,
            open: 10 * PRICE_SCALE,
            high: 11 * PRICE_SCALE,
            low: 9 * PRICE_SCALE,
            close: 10 * PRICE_SCALE,
            volume: 1,
        };

        md.add_bar("NIFTY04JAN2421900CE", bar);

        let instrument = md.get_instrument(1).expect("missing instrument metadata");
        assert_eq!(instrument.kind, InstrumentKind::Option);
        let option = instrument.option.as_ref().expect("missing option spec");
        assert_eq!(option.underlying, "NIFTY");
        assert_eq!(option.expiry_yyyymmdd, 20240104);
        assert_eq!(option.strike, 21900 * PRICE_SCALE);
        assert_eq!(option.option_type, OptionType::Call);
    }
}
