use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cache::store::{CacheStatus, EnsureLoadedResult};

#[derive(Debug, Clone)]
pub enum CacheRequest {
    Ping,
    Status,
    Ensure {
        path: String,
        start_ts: Option<i64>,
        end_ts: Option<i64>,
    },
    Evict {
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictResult {
    pub removed: bool,
}

pub fn decode_request(line: &str) -> Result<CacheRequest> {
    if line.eq_ignore_ascii_case("PING") {
        return Ok(CacheRequest::Ping);
    }
    if line.eq_ignore_ascii_case("STATUS") {
        return Ok(CacheRequest::Status);
    }
    if let Some(path) = line.strip_prefix("ENSURE ") {
        let raw = path.trim();
        let mut tokens = raw.split_whitespace();
        let path = tokens
            .next()
            .ok_or_else(|| anyhow::anyhow!("ENSURE requires a dataset path"))?
            .to_string();
        let start_ts = tokens.next().and_then(|token| token.parse::<i64>().ok());
        let end_ts = tokens.next().and_then(|token| token.parse::<i64>().ok());
        return Ok(CacheRequest::Ensure {
            path,
            start_ts,
            end_ts,
        });
    }
    if let Some(path) = line.strip_prefix("EVICT ") {
        return Ok(CacheRequest::Evict {
            path: path.trim().to_string(),
        });
    }

    Err(anyhow::anyhow!("unsupported command"))
}

pub fn encode_pong() -> String {
    "PONG".to_string()
}

pub fn encode_status_ok(status: &CacheStatus) -> Result<String> {
    Ok(format!("OK {}", serde_json::to_string(status)?))
}

pub fn encode_ensure_loaded_ok(result: &EnsureLoadedResult) -> Result<String> {
    Ok(format!("OK {}", serde_json::to_string(result)?))
}

pub fn encode_evict_ok(removed: bool) -> String {
    let payload = EvictResult { removed };
    format!(
        "OK {}",
        serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
    )
}

pub fn encode_error(message: &str) -> String {
    format!("ERR {message}")
}

pub fn ensure_loaded(
    addr: &str,
    path: &str,
    range: Option<(i64, i64)>,
) -> Result<EnsureLoadedResult> {
    let ensure_timeout_ms = std::env::var("BT_CACHE_ENSURE_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(600_000); // 10 min default: large parquet loads can easily exceed 2 min
    let command = if let Some((start_ts, end_ts)) = range {
        format!("ENSURE {} {} {}", path, start_ts, end_ts)
    } else {
        format!("ENSURE {}", path)
    };
    let response = send_command(addr, &command, Some(ensure_timeout_ms))?;
    parse_ok_json::<EnsureLoadedResult>(&response)
}

pub fn status(addr: &str) -> Result<CacheStatus> {
    let response = send_command(addr, "STATUS", None)?;
    parse_ok_json::<CacheStatus>(&response)
}

pub fn evict(addr: &str, path: &str) -> Result<EvictResult> {
    let response = send_command(addr, &format!("EVICT {}", path), None)?;
    parse_ok_json::<EvictResult>(&response)
}

fn send_command(addr: &str, command: &str, timeout_override_ms: Option<u64>) -> Result<String> {
    let timeout_ms = timeout_override_ms.unwrap_or_else(|| {
        std::env::var("BT_CACHE_TIMEOUT_MS")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1_500)
    });
    let retry_count = std::env::var("BT_CACHE_RETRIES")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .unwrap_or(3);
    let retry_delay_ms = std::env::var("BT_CACHE_RETRY_DELAY_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .unwrap_or(100);
    let timeout = Duration::from_millis(timeout_ms);

    let mut attempt = 0u32;
    loop {
        attempt += 1;

        let mut last_error: Option<std::io::Error> = None;
        let mut stream_opt: Option<TcpStream> = None;
        for socket_addr in addr
            .to_socket_addrs()
            .with_context(|| format!("invalid cache server address: {addr}"))?
        {
            match TcpStream::connect_timeout(&socket_addr, timeout) {
                Ok(stream) => {
                    stream_opt = Some(stream);
                    break;
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        let mut stream = match stream_opt {
            Some(stream) => stream,
            None => {
                let detail = last_error
                    .map(|err| err.to_string())
                    .unwrap_or_else(|| "no resolved socket addresses".to_string());
                if attempt <= retry_count {
                    std::thread::sleep(Duration::from_millis(retry_delay_ms));
                    continue;
                }
                return Err(anyhow::anyhow!(format!(
                    "failed to connect to cache server at {addr}: {detail}"
                )));
            }
        };

        stream
            .set_read_timeout(Some(timeout))
            .with_context(|| format!("failed to set cache read timeout for {addr}"))?;
        stream
            .set_write_timeout(Some(timeout))
            .with_context(|| format!("failed to set cache write timeout for {addr}"))?;

        if let Err(err) = writeln!(stream, "{command}").and_then(|_| stream.flush()) {
            if attempt <= retry_count {
                std::thread::sleep(Duration::from_millis(retry_delay_ms));
                continue;
            }
            return Err(anyhow::anyhow!(format!(
                "cache command write failed for {addr}: {err}"
            )));
        }

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    if attempt <= retry_count {
                        std::thread::sleep(Duration::from_millis(retry_delay_ms));
                        continue;
                    }
                    return Err(anyhow::anyhow!("empty response from cache server"));
                }
                return Ok(trimmed.to_string());
            }
            Err(err) => {
                if attempt <= retry_count {
                    std::thread::sleep(Duration::from_millis(retry_delay_ms));
                    continue;
                }
                return Err(anyhow::anyhow!(format!(
                    "cache command read failed for {addr}: {err}"
                )));
            }
        }
    }
}

fn parse_ok_json<T>(response: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if let Some(err) = response.strip_prefix("ERR ") {
        return Err(anyhow::anyhow!(err.to_string()));
    }
    let payload = response
        .strip_prefix("OK ")
        .ok_or_else(|| anyhow::anyhow!(format!("unexpected response: {response}")))?;
    Ok(serde_json::from_str::<T>(payload)?)
}
