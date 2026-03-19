use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

use chrono::{Datelike, FixedOffset, TimeZone, Timelike, Utc};

/// Price is represented as scaled integer (i64) with factor 10,000.
/// e.g. 150.50 -> 1,505,000
pub type Price = i64;
pub type InstrumentId = u32;
pub const PRICE_SCALE: i64 = 10_000;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub enum MarketTimeZone {
    Utc,
    AsiaKolkata,
}

impl MarketTimeZone {
    pub fn fixed_offset(self) -> FixedOffset {
        match self {
            MarketTimeZone::Utc => FixedOffset::east_opt(0).expect("valid utc offset"),
            MarketTimeZone::AsiaKolkata => {
                FixedOffset::east_opt(5 * 3600 + 30 * 60).expect("valid ist offset")
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MarketTimeZone::Utc => "UTC",
            MarketTimeZone::AsiaKolkata => "Asia/Kolkata",
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct SimTime {
    pub epoch_seconds: i64,
    pub tz: MarketTimeZone,
}

impl SimTime {
    pub fn new(epoch_seconds: i64, tz: MarketTimeZone) -> Self {
        Self { epoch_seconds, tz }
    }

    pub fn utc(epoch_seconds: i64) -> Self {
        Self::new(epoch_seconds, MarketTimeZone::Utc)
    }

    pub fn ist(epoch_seconds: i64) -> Self {
        Self::new(epoch_seconds, MarketTimeZone::AsiaKolkata)
    }

    pub fn as_epoch_seconds(self) -> i64 {
        self.epoch_seconds
    }

    pub fn with_tz(self, tz: MarketTimeZone) -> Self {
        Self {
            epoch_seconds: self.epoch_seconds,
            tz,
        }
    }

    pub fn local_date_key(self) -> i32 {
        let dt = self.local_datetime();
        dt.year() * 10_000 + dt.month() as i32 * 100 + dt.day() as i32
    }

    pub fn seconds_from_midnight(self) -> i64 {
        let dt = self.local_datetime();
        (dt.hour() as i64) * 3600 + (dt.minute() as i64) * 60 + dt.second() as i64
    }

    pub fn div_euclid(self, rhs: i64) -> i64 {
        if rhs == 86_400 {
            self.local_date_key() as i64
        } else {
            self.epoch_seconds.div_euclid(rhs)
        }
    }

    pub fn rem_euclid(self, rhs: i64) -> i64 {
        if rhs == 86_400 {
            self.seconds_from_midnight()
        } else {
            self.epoch_seconds.rem_euclid(rhs)
        }
    }

    pub fn start_of_local_day(self) -> Self {
        let dt = self.local_datetime();
        let date = dt.date_naive();
        let naive_midnight = date.and_hms_opt(0, 0, 0).expect("midnight should be valid");
        let local_midnight = self
            .tz
            .fixed_offset()
            .from_local_datetime(&naive_midnight)
            .single()
            .expect("fixed offset has unique local time");
        Self {
            epoch_seconds: local_midnight.timestamp(),
            tz: self.tz,
        }
    }

    pub fn add_seconds(self, seconds: i64) -> Self {
        Self {
            epoch_seconds: self.epoch_seconds.saturating_add(seconds),
            tz: self.tz,
        }
    }

    pub fn local_datetime(self) -> chrono::DateTime<FixedOffset> {
        let utc = chrono::DateTime::<Utc>::from_timestamp(self.epoch_seconds, 0)
            .expect("timestamp out of range");
        utc.with_timezone(&self.tz.fixed_offset())
    }

    pub fn format_rfc3339(self) -> String {
        self.local_datetime().to_rfc3339()
    }
}

impl Default for SimTime {
    fn default() -> Self {
        Self::utc(0)
    }
}

impl PartialOrd for SimTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SimTime {
    fn cmp(&self, other: &Self) -> Ordering {
        self.epoch_seconds.cmp(&other.epoch_seconds)
    }
}

impl fmt::Display for SimTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.local_datetime().format("%Y-%m-%d %H:%M:%S %:z")
        )
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub enum OptionType {
    Call,
    Put,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub enum InstrumentKind {
    Spot,
    Future,
    Option,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }

    pub fn sign(&self) -> i32 {
        match self {
            Side::Buy => 1,
            Side::Sell => -1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit(Price),
    Stop(Price),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Status {
    Pending,
    Submitted,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeInForce {
    Day,
    GTC,
    IOC,
}
