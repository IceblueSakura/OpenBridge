//! Bootstrap TOML 的解析与运行时边界校验。

use std::{net::SocketAddr, time::Duration};

use super::{
    BOOTSTRAP_SCHEMA_VERSION, BootstrapConfig, BootstrapConfigError, HttpClientConfig,
    RuntimeLimits, document::RawBootstrap,
};

/// 解析并校验 bootstrap TOML。
///
/// 该函数只产生启动配置，不会注册 provider、model、target、upstream API 或 route。
pub fn parse_bootstrap_config(document: &str) -> Result<BootstrapConfig, BootstrapConfigError> {
    // 解析 bootstrap 文档并确认 schema 版本。
    let raw: RawBootstrap = toml::from_str(document).map_err(|_| BootstrapConfigError::Parse)?;
    if raw.schema_version != BOOTSTRAP_SCHEMA_VERSION {
        return Err(BootstrapConfigError::UnsupportedSchema {
            actual: raw.schema_version,
        });
    }
    // 校验所有内存、超时和连接池限制均可提供有效边界。
    validate_nonzero("max_request_body_bytes", raw.max_request_body_bytes)?;
    validate_nonzero("max_sse_event_bytes", raw.max_sse_event_bytes)?;
    validate_nonzero(
        "upstream_connect_timeout_ms",
        raw.upstream_connect_timeout_ms,
    )?;
    validate_nonzero(
        "upstream_pool_idle_timeout_ms",
        raw.upstream_pool_idle_timeout_ms,
    )?;
    validate_nonzero(
        "upstream_pool_max_idle_per_host",
        raw.upstream_pool_max_idle_per_host,
    )?;
    // 解析并限制监听地址为 loopback，避免 bootstrap 直接暴露服务。
    let listen = raw
        .listen
        .parse::<SocketAddr>()
        .ok()
        .filter(|address| address.ip().is_loopback())
        .ok_or_else(|| BootstrapConfigError::NonLoopbackListen {
            listen: raw.listen.clone(),
        })?;

    // 将原始字段转换成运行时值对象。
    Ok(BootstrapConfig {
        listen,
        users_file: raw.users_file,
        upstream_credentials_file: raw.upstream_credentials_file,
        limits: RuntimeLimits {
            max_request_body_bytes: raw.max_request_body_bytes,
            max_sse_event_bytes: raw.max_sse_event_bytes,
        },
        http_client: HttpClientConfig {
            connect_timeout: Duration::from_millis(raw.upstream_connect_timeout_ms),
            pool_idle_timeout: Duration::from_millis(raw.upstream_pool_idle_timeout_ms),
            pool_max_idle_per_host: raw.upstream_pool_max_idle_per_host,
        },
    })
}

/// 拒绝零值配置，统一保护内存、时间和连接池边界。
fn validate_nonzero(
    name: &'static str,
    value: impl Copy + PartialEq + From<u8>,
) -> Result<(), BootstrapConfigError> {
    // 统一拒绝零值，保证后续内存和时间边界不会退化为无效配置。
    if value == 0.into() {
        Err(BootstrapConfigError::InvalidLimit { name })
    } else {
        Ok(())
    }
}
