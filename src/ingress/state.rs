//! HTTP ingress 使用的共享服务状态。

use std::sync::Arc;

use crate::{
    credential::CredentialStore, identity::UserRegistry, observability::GatewayMetrics,
    registry::RuntimeRegistry, transport::upstream::UpstreamTransport,
};

use super::{credential_health::CredentialHealth, health::TargetHealth};

/// handler 依赖的不可变服务句柄。
///
/// 编译期注册表在启动后保持不可变；上游 transport 与 credential source 以 trait/值对象
/// 注入，因此 contract test 可以验证 HTTP/SSE 边界而无需真实 provider 或明文环境 secret。
#[derive(Clone)]
pub struct GatewayState {
    pub(super) registry: Arc<RuntimeRegistry>,
    pub(super) upstream: Arc<dyn UpstreamTransport>,
    pub(super) users: Arc<UserRegistry>,
    pub(super) credentials: Arc<CredentialStore>,
    pub(super) health: Arc<TargetHealth>,
    pub(super) credential_health: Arc<CredentialHealth>,
    pub(super) metrics: GatewayMetrics,
}

impl GatewayState {
    /// 创建可注入 transport 与 credential source 的服务状态。
    pub fn new(
        registry: Arc<RuntimeRegistry>,
        upstream: Arc<dyn UpstreamTransport>,
        users: Arc<UserRegistry>,
        credentials: Arc<CredentialStore>,
    ) -> Self {
        Self {
            registry,
            upstream,
            users,
            credentials,
            health: Arc::new(TargetHealth::default()),
            credential_health: Arc::new(CredentialHealth::default()),
            metrics: GatewayMetrics::default(),
        }
    }

    /// 返回共享的进程内低基数累计值句柄，供 exporter 或测试读取快照。
    pub fn metrics(&self) -> GatewayMetrics {
        self.metrics.clone()
    }
}
