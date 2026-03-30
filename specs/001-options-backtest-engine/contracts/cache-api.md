# Cache API Contract (US6)

## Transport
- Protocol: TCP line protocol
- Default bind: 127.0.0.1:7878
- Request/response: one command per line, one response line

## Commands

### PING
Request:
PING

Response:
PONG

### STATUS
Request:
STATUS

Response:
OK <json>

JSON shape:
{
  "entry_count": 1,
  "keys": ["/path/file.parquet:123:1700000000"]
}

### ENSURE <path>
Request:
ENSURE /abs/or/rel/path/to/data.parquet

Response:
OK <json>

JSON shape:
{
  "entry": {
    "fingerprint": {
      "canonical_path": "/abs/path/data.parquet",
      "size_bytes": 123,
      "modified_unix_secs": 1700000000,
      "sha256": null
    },
    "loaded_at_unix_secs": 1700000012,
    "instrument_count": 100,
    "bar_count": 500000
  },
  "cache_hit": false,
  "load_ms": 320
}

### EVICT <path>
Request:
EVICT /abs/or/rel/path/to/data.parquet

Response:
OK <json>

JSON shape:
{
  "removed": true
}

## Error Contract
- All errors return a single line with prefix:
ERR <message>

## bt run / bt sweep behavior
- Attempt ENSURE before execution.
- If reachable, use ENSURE result telemetry:
  - cache_hit
  - load_ms
- If unreachable, continue with direct load path and emit warning.
