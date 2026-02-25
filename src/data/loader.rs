use std::path::Path;
use std::sync::Arc;
use anyhow::{Result, Context};
use polars::prelude::*;
use crate::data::models::{MarketData, Bar};
// use crate::common::types::PRICE_SCALE;

pub struct DataLoader;

impl DataLoader {
    pub fn load_parquet(path: &str) -> Result<Arc<MarketData>> {
        let file_path = Path::new(path);
        if !file_path.exists() {
            return Err(anyhow::anyhow!("Data file not found: {}", path));
        }

        let _df = LazyFrame::scan_parquet(file_path, Default::default())?
            .collect()?;
            
        let md = MarketData::new();
        Ok(Arc::new(md))
    }
}
