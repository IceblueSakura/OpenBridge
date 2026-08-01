//! 单进程内跨请求上游健康隔离。
//!
//! health key 只能来自启动时已校验的 `quota_scope`、`fault_domain` 或 target id，业务请求
//! 不能创建或覆盖 scope。本模块只保存有界 cooldown 截止时间，不做动态权重、持久化、
//! 分布式协调、credential 轮换或后台探测。

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

use http::{HeaderMap, header::RETRY_AFTER};

use crate::{provider::UpstreamErrorKind, registry::UpstreamTarget};

const DEFAULT_COOLDOWN: Duration = Duration::from_secs(1);
const MAX_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum HealthScope {
    Quota(String),
    Fault(String),
}

/// 所有 GatewayState clone 共享的短时 cooldown 表。
#[derive(Debug, Default)]
pub(super) struct TargetHealth {
    deadlines: Mutex<HashMap<HealthScope, Instant>>,
}

impl TargetHealth {
    /// 判断一个新无状态请求是否可选择该 target。
    pub(super) fn is_available(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        now: Instant,
    ) -> bool {
        // 生成受注册表约束的 quota 与 fault key。
        let scopes = Self::target_scopes(target_id, target);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // 清理已过期条目，并在任一边界仍冷却时拒绝新选择。
        deadlines.retain(|_, deadline| *deadline > now);
        scopes.iter().all(|scope| !deadlines.contains_key(scope))
    }

    /// 根据 adapter 的 HTTP failure 类别记录对应隔离边界。
    pub(super) fn record_http_failure(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        kind: UpstreamErrorKind,
        headers: &HeaderMap,
        now: Instant,
    ) {
        // 只把限流映射到 quota，把暂时不可用映射到 fault domain。
        let scope = match kind {
            UpstreamErrorKind::RateLimited => Self::quota_scope(target_id, target),
            UpstreamErrorKind::UpstreamUnavailable => Self::fault_scope(target_id, target),
            UpstreamErrorKind::InvalidRequest
            | UpstreamErrorKind::Authentication
            | UpstreamErrorKind::UpstreamFailure => return,
        };
        let delay = retry_after_delay(headers).unwrap_or(DEFAULT_COOLDOWN);
        self.record(scope, now, delay);
    }

    /// 将 timeout/transport failure 记录到 fault domain。
    pub(super) fn record_transport_failure(
        &self,
        target_id: &str,
        target: &UpstreamTarget,
        now: Instant,
    ) {
        self.record(Self::fault_scope(target_id, target), now, DEFAULT_COOLDOWN);
    }

    /// 成功响应清除该 target 所属的 quota 与 fault cooldown。
    pub(super) fn record_success(&self, target_id: &str, target: &UpstreamTarget) {
        // 只清除该 target 显式所属的两个边界，不影响其他注册 scope。
        let scopes = Self::target_scopes(target_id, target);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for scope in scopes {
            deadlines.remove(&scope);
        }
    }

    fn record(&self, scope: HealthScope, now: Instant, delay: Duration) {
        // 限制 Provider 建议的 cooldown，避免异常 header 长期封锁进程内 route。
        let deadline = now + delay.min(MAX_COOLDOWN);
        let mut deadlines = self
            .deadlines
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        deadlines
            .entry(scope)
            .and_modify(|current| *current = (*current).max(deadline))
            .or_insert(deadline);
    }

    fn target_scopes(target_id: &str, target: &UpstreamTarget) -> [HealthScope; 2] {
        [
            Self::quota_scope(target_id, target),
            Self::fault_scope(target_id, target),
        ]
    }

    fn quota_scope(target_id: &str, target: &UpstreamTarget) -> HealthScope {
        HealthScope::Quota(target.quota_scope().unwrap_or(target_id).to_owned())
    }

    fn fault_scope(target_id: &str, target: &UpstreamTarget) -> HealthScope {
        HealthScope::Fault(target.fault_domain().unwrap_or(target_id).to_owned())
    }
}

fn retry_after_delay(headers: &HeaderMap) -> Option<Duration> {
    // 优先解析 delta-seconds，再解析标准 HTTP-date。
    let value = headers.get(RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let retry_at = httpdate::parse_http_date(value).ok()?;
    Some(
        retry_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO),
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use http::{HeaderMap, HeaderValue};

    use super::retry_after_delay;

    #[test]
    fn retry_after_accepts_seconds_and_http_date() {
        // 验证两种标准表示都转换为有界 cooldown 输入。
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("3"));
        assert_eq!(retry_after_delay(&headers), Some(Duration::from_secs(3)));

        headers.insert(
            "retry-after",
            HeaderValue::from_static("Wed, 21 Oct 2037 07:28:00 GMT"),
        );
        assert!(retry_after_delay(&headers).is_some_and(|delay| !delay.is_zero()));
    }
}
