//! Private document model used only to deserialize bootstrap TOML.
//!
//! Provider, Model, Upstream Target, Upstream API, Route, and Public Model entries are registered
//! in Rust code and are not runtime configuration.

use bytesize::ByteSize;
use serde::{Deserialize, Deserializer, de::Error as _};
use std::{path::PathBuf, time::Duration};

/// Strict string-backed byte size parsed by the mature `bytesize` grammar.
pub(super) struct RawByteSize(ByteSize);

impl RawByteSize {
    /// Returns the parsed byte count before runtime-width validation.
    pub(super) fn as_u64(&self) -> u64 {
        self.0.as_u64()
    }
}

impl<'de> Deserialize<'de> for RawByteSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse_byte_size(&value).map(Self).map_err(D::Error::custom)
    }
}

/// Parses one positive-integer byte size with an explicit, case-sensitive SI or IEC suffix.
fn parse_byte_size(value: &str) -> Result<ByteSize, String> {
    let unit_offset = value
        .find(|character: char| !character.is_ascii_digit())
        .ok_or_else(|| "byte size must include an explicit unit".to_owned())?;
    let (quantity, unit) = value.split_at(unit_offset);
    if quantity.is_empty()
        || !matches!(
            unit,
            "B" | "KB"
                | "MB"
                | "GB"
                | "TB"
                | "PB"
                | "EB"
                | "KiB"
                | "MiB"
                | "GiB"
                | "TiB"
                | "PiB"
                | "EiB"
        )
    {
        return Err("byte size must use an explicit SI or IEC byte unit".to_owned());
    }

    let quantity = quantity
        .parse::<u64>()
        .map_err(|_| "byte-size quantity is out of range".to_owned())?;
    let one_unit = format!("1{unit}")
        .parse::<ByteSize>()
        .map_err(|error| format!("invalid byte-size unit: {error}"))?;
    let bytes = quantity
        .checked_mul(one_unit.as_u64())
        .ok_or_else(|| "byte-size value is out of range".to_owned())?;
    Ok(ByteSize::b(bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBootstrap {
    pub(super) schema_version: u32,
    pub(super) listen: String,
    pub(super) users_file: PathBuf,
    pub(super) upstream_credentials_file: PathBuf,
    pub(super) default_instructions: Option<String>,
    pub(super) max_request_body: RawByteSize,
    pub(super) max_json_response_body: RawByteSize,
    pub(super) max_replay_body: RawByteSize,
    pub(super) max_sse_event: RawByteSize,
    #[serde(with = "humantime_serde")]
    pub(super) upstream_connect_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub(super) upstream_pool_idle_timeout: Duration,
    pub(super) upstream_pool_max_idle_per_host: usize,
    pub(super) logging: Option<RawHttpLogging>,
    pub(super) telemetry: Option<RawTelemetry>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct RawHttpLogging {
    pub(super) http_jsonl_directory: Option<std::path::PathBuf>,
    pub(super) request_headers: bool,
    pub(super) request_body: bool,
    pub(super) response_headers: bool,
    pub(super) response_body: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTelemetry {
    pub(super) traces: Option<RawOtlpHttpExport>,
    pub(super) metrics: Option<RawOtlpHttpExport>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawOtlpHttpExport {
    pub(super) otlp_http_endpoint: String,
}
