use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
pub mod ipc;
pub mod snapshot;
pub mod store;

use crate::cache::ipc::{
    decode_request,
    encode_ensure_loaded_ok,
    encode_error,
    encode_evict_ok,
    encode_pong,
    encode_status_ok,
    CacheRequest,
};
use crate::cache::store::CacheStore;

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

        if let Err(err) = handle_connection(&mut stream, &state) {
            let _ = writeln!(stream, "{}", encode_error(&err.to_string()));
            let _ = stream.flush();
        }
    }

    Ok(())
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
        } => {
            let mut guard = state
                .lock()
                .map_err(|_| anyhow::anyhow!("cache state lock poisoned"))?;
            let result = guard.ensure_loaded(&path, start_ts, end_ts)?;
            encode_ensure_loaded_ok(&result)?
        }
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
