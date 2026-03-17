use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
pub mod ipc;
pub mod snapshot;
pub mod store;

use crate::cache::ipc::{
    decode_request, encode_ensure_loaded_ok, encode_error, encode_evict_ok, encode_pong,
    encode_status_ok, CacheRequest,
};
use crate::cache::snapshot::{
    build_shared_snapshot_handle, load_market_data_snapshot, read_snapshot_metadata,
    snapshot_path_for_key, write_market_data_snapshot, write_snapshot_metadata, SnapshotMetadata,
};
use crate::cache::store::{CacheEntry, CacheStore};
use crate::data::fingerprint::build_dataset_fingerprint;
use crate::data::loader::DataLoader;

#[derive(Debug, Clone)]
pub struct CacheServerConfig {
    pub bind_addr: String,
    pub include_sha256: bool,
}

pub fn run_cache_server(config: CacheServerConfig) -> Result<()> {
    let listener = TcpListener::bind(&config.bind_addr)
        .with_context(|| format!("failed to bind cache server on {}", config.bind_addr))?;

    eprintln!("bt cache-server listening on {}", config.bind_addr);
    let state = Arc::new(Mutex::new(CacheStore::new(config.include_sha256)));

    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(err) => {
                eprintln!("bt cache-server accept error: {err}");
                continue;
            }
        };

        let state_for_conn = Arc::clone(&state);
        std::thread::spawn(move || {
            if let Err(err) = handle_connection(&mut stream, &state_for_conn) {
                let _ = writeln!(stream, "{}", encode_error(&err.to_string()));
                let _ = stream.flush();
            }
        });
    }

    Ok(())
}

fn cache_debug_progress_enabled() -> bool {
    std::env::var("BT_CACHE_DEBUG_PROGRESS")
        .ok()
        .map(|raw| {
            matches!(
                raw.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn handle_connection(stream: &mut TcpStream, state: &Arc<Mutex<CacheStore>>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone().context("failed to clone TCP stream")?);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let request = decode_request(line.trim())?;
    let response = match request {
        CacheRequest::Ping => encode_pong(),
        CacheRequest::Status => {
            let guard = state
                .lock()
                .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
            encode_status_ok(&guard.status())?
        }
        CacheRequest::Ensure {
            path,
            start_ts,
            end_ts,
        } => encode_ensure_loaded_ok(&process_ensure_request(state, &path, start_ts, end_ts)?)?,
        CacheRequest::Evict { path } => {
            let mut guard = state
                .lock()
                .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
            encode_evict_ok(guard.evict(&path)?)
        }
    };

    writeln!(stream, "{response}")?;
    stream.flush()?;
    Ok(())
}

fn process_ensure_request(
    state: &Arc<Mutex<CacheStore>>,
    path: &str,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
) -> Result<crate::cache::store::EnsureLoadedResult> {
    let debug_progress = cache_debug_progress_enabled();
    let ensure_started = Instant::now();
    if debug_progress {
        eprintln!(
            "bt cache-server ensure start path={} range={:?}..{:?}",
            path, start_ts, end_ts
        );
    }

    let include_sha256 = {
        let guard = state
            .lock()
            .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
        guard.include_sha256()
    };

    let fingerprint_started = Instant::now();
    let fingerprint = build_dataset_fingerprint(std::path::Path::new(path), include_sha256)?;
    let key = if let (Some(start), Some(end)) = (start_ts, end_ts) {
        format!("{}:{}:{}", fingerprint.key(), start, end)
    } else {
        fingerprint.key()
    };
    if debug_progress {
        eprintln!(
            "bt cache-server ensure fingerprint_done key={} elapsed_ms={} total_ms={}",
            key,
            fingerprint_started.elapsed().as_millis(),
            ensure_started.elapsed().as_millis()
        );
    }

    {
        let guard = state
            .lock()
            .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
        if let Some(hit) = guard.lookup_by_key(&key) {
            if debug_progress {
                eprintln!(
                    "bt cache-server ensure cache_hit key={} total_ms={}",
                    key,
                    ensure_started.elapsed().as_millis()
                );
            }
            return Ok(hit);
        }
    }

    let snapshot_restore_started = Instant::now();
    let snapshot_path = snapshot_path_for_key(&key);
    if snapshot_path.exists() {
        let metadata = match read_snapshot_metadata(&snapshot_path)? {
            Some(metadata) => metadata,
            None => {
                let restored_data = load_market_data_snapshot(&snapshot_path)?;
                let bar_count = restored_data
                    .bars
                    .values()
                    .map(|bars| bars.len())
                    .sum::<usize>();
                let metadata = SnapshotMetadata {
                    loaded_at_unix_secs: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    instrument_count: restored_data.instruments.len(),
                    bar_count,
                };
                write_snapshot_metadata(&snapshot_path, &metadata)?;
                metadata
            }
        };

        let restore_ms = snapshot_restore_started.elapsed().as_millis();

        let generation = {
            let mut guard = state
                .lock()
                .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
            guard.reserve_generation()
        };
        let shared_handle = build_shared_snapshot_handle(&snapshot_path, generation)?;

        let entry = CacheEntry {
            fingerprint,
            start_ts,
            end_ts,
            loaded_at_unix_secs: metadata.loaded_at_unix_secs,
            instrument_count: metadata.instrument_count,
            bar_count: metadata.bar_count,
            snapshot_path: snapshot_path.to_string_lossy().to_string(),
        };

        let mut guard = state
            .lock()
            .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
        let result = guard.insert_prepared_entry(key, entry, shared_handle, restore_ms)?;

        if debug_progress {
            eprintln!(
                "bt cache-server ensure snapshot_reused key={} restore_ms={} total_ms={}",
                result.entry.fingerprint.key(),
                restore_ms,
                ensure_started.elapsed().as_millis()
            );
        }

        return Ok(result);
    }

    let started = Instant::now();
    let loaded_data = if let (Some(start), Some(end)) = (start_ts, end_ts) {
        DataLoader::load_parquet_range(path, start, end)?
    } else {
        DataLoader::load_parquet(path)?
    };
    let load_ms = started.elapsed().as_millis();
    if debug_progress {
        eprintln!(
            "bt cache-server ensure load_done key={} load_ms={} total_ms={}",
            key,
            load_ms,
            ensure_started.elapsed().as_millis()
        );
    }

    let bar_count = loaded_data
        .bars
        .values()
        .map(|bars| bars.len())
        .sum::<usize>();

    let snapshot_started = Instant::now();
    let snapshot_path = write_market_data_snapshot(&key, loaded_data.as_ref())?;
    if debug_progress {
        eprintln!(
            "bt cache-server ensure snapshot_written key={} snapshot={} bars={} instruments={} elapsed_ms={} total_ms={}",
            key,
            snapshot_path.display(),
            bar_count,
            loaded_data.instruments.len(),
            snapshot_started.elapsed().as_millis(),
            ensure_started.elapsed().as_millis()
        );
    }

    let handle_started = Instant::now();
    let generation = {
        let mut guard = state
            .lock()
            .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
        guard.reserve_generation()
    };
    let shared_handle = build_shared_snapshot_handle(&snapshot_path, generation)?;
    if debug_progress {
        eprintln!(
            "bt cache-server ensure handle_built key={} generation={} elapsed_ms={} total_ms={}",
            key,
            generation,
            handle_started.elapsed().as_millis(),
            ensure_started.elapsed().as_millis()
        );
    }

    let entry = CacheEntry {
        fingerprint,
        start_ts,
        end_ts,
        loaded_at_unix_secs: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        instrument_count: loaded_data.instruments.len(),
        bar_count,
        snapshot_path: snapshot_path.to_string_lossy().to_string(),
    };

    write_snapshot_metadata(
        &snapshot_path,
        &SnapshotMetadata {
            loaded_at_unix_secs: entry.loaded_at_unix_secs,
            instrument_count: entry.instrument_count,
            bar_count: entry.bar_count,
        },
    )?;

    let mut guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
    let result = guard.insert_prepared_entry(key, entry, shared_handle, load_ms)?;
    if debug_progress {
        eprintln!(
            "bt cache-server ensure inserted key={} cache_hit={} total_ms={}",
            result.entry.fingerprint.key(),
            result.cache_hit,
            ensure_started.elapsed().as_millis()
        );
    }
    Ok(result)
}
