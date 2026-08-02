//! Provider credential pool 的进程内 round-robin 与成员 cooldown。
//!
//! 状态键只包含注册表 pool id、非敏感成员 id 与 generation。模块不持有 secret，不解析
//! 错误 body，也不把某个成员的 429 扩散为 Provider 或 target 整体故障。

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Instant,
};

use http::HeaderMap;

use crate::credential::UpstreamCredential;

use super::health::{DEFAULT_COOLDOWN, MAX_COOLDOWN, retry_after_delay};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct MemberKey {
    member_id: String,
    generation: u64,
}

#[derive(Debug, Default)]
struct PoolState {
    cursor: usize,
    cooldowns: HashMap<MemberKey, Instant>,
}

/// 所有 GatewayState clone 共享的 pool 选择与 cooldown 状态。
#[derive(Debug, Default)]
pub(super) struct CredentialHealth {
    pools: Mutex<HashMap<String, PoolState>>,
}

impl CredentialHealth {
    /// 按共享 cursor 选择一个未冷却且未在本请求中拒绝的成员索引。
    pub(super) fn select_member(
        &self,
        pool_id: &str,
        members: &[UpstreamCredential<'_>],
        rejected: &HashSet<String>,
        now: Instant,
    ) -> Option<usize> {
        // 清理过期 cooldown 并从共享 cursor 开始扫描一圈。
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        state.cooldowns.retain(|_, deadline| *deadline > now);
        for offset in 0..members.len() {
            let index = (state.cursor + offset) % members.len();
            let member = &members[index];
            let key = member_key(member);
            if rejected.contains(member.member_id()) || state.cooldowns.contains_key(&key) {
                continue;
            }

            // 选中后立即推进 cursor，使并发请求自然分散到后续成员。
            state.cursor = (index + 1) % members.len();
            return Some(index);
        }
        None
    }

    /// 判断 pool 是否还有本请求可尝试的健康成员，不推进 round-robin cursor。
    pub(super) fn has_available_member(
        &self,
        pool_id: &str,
        members: &[UpstreamCredential<'_>],
        rejected: &HashSet<String>,
        now: Instant,
    ) -> bool {
        // 清理过期条目后只执行无副作用可用性检查。
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        state.cooldowns.retain(|_, deadline| *deadline > now);
        members.iter().any(|member| {
            !rejected.contains(member.member_id())
                && !state.cooldowns.contains_key(&member_key(member))
        })
    }

    /// 根据 429 的 `Retry-After` 为单个成员记录有界跨请求 cooldown。
    pub(super) fn record_rate_limited(
        &self,
        pool_id: &str,
        member: &UpstreamCredential<'_>,
        headers: &HeaderMap,
        now: Instant,
    ) {
        // 缺失或非法 header 使用统一默认值，并把异常长建议限制到硬上限。
        let delay = retry_after_delay(headers)
            .unwrap_or(DEFAULT_COOLDOWN)
            .min(MAX_COOLDOWN);
        let deadline = now + delay;
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let state = pools.entry(pool_id.to_owned()).or_default();
        let key = member_key(member);
        state
            .cooldowns
            .entry(key)
            .and_modify(|current| *current = (*current).max(deadline))
            .or_insert(deadline);
    }

    /// 成功响应只清除当前成员的 cooldown，不影响同 pool 其他成员。
    pub(super) fn record_success(&self, pool_id: &str, member: &UpstreamCredential<'_>) {
        // 精确删除包含 generation 的成员键，避免旧状态污染新快照。
        let mut pools = self
            .pools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(state) = pools.get_mut(pool_id) {
            state.cooldowns.remove(&member_key(member));
        }
    }
}

fn member_key(member: &UpstreamCredential<'_>) -> MemberKey {
    MemberKey {
        member_id: member.member_id().to_owned(),
        generation: member.metadata().generation(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use http::{HeaderMap, HeaderValue};
    use secrecy::SecretString;

    use super::CredentialHealth;
    use crate::{
        credential::{
            CredentialMetadata, CredentialSource, CredentialStore, CredentialStoreBuilder,
        },
        provider::{CredentialKind, ProviderKind},
    };

    fn credential_store(generation: u64) -> CredentialStore {
        let mut builder = CredentialStoreBuilder::new();
        for (index, secret) in ["key-a", "key-b"].into_iter().enumerate() {
            builder
                .insert_upstream_member(
                    ProviderKind::OpenAi,
                    "openai-primary",
                    format!("openai-primary#{}", index + 1),
                    SecretString::from(secret),
                    CredentialMetadata::upstream(
                        CredentialKind::ApiKey,
                        CredentialSource::Programmatic,
                    )
                    .with_generation(generation),
                )
                .unwrap();
        }
        builder.build()
    }

    #[test]
    fn selector_advances_one_shared_round_robin_cursor() {
        // 连续选择在同一个 pool 状态上确定性推进，不需要读取 secret。
        let store = credential_store(1);
        let members = store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let selector = CredentialHealth::default();
        let rejected = HashSet::new();
        let now = std::time::Instant::now();
        assert_eq!(
            selector.select_member("openai-primary", &members, &rejected, now),
            Some(0)
        );
        assert_eq!(
            selector.select_member("openai-primary", &members, &rejected, now),
            Some(1)
        );
    }

    #[test]
    fn cooldown_is_capped_and_generation_changes_isolate_old_state() {
        // 超长 Retry-After 最多冷却 30 秒，期间仍可选择健康 peer。
        let store = credential_store(1);
        let members = store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let selector = CredentialHealth::default();
        let rejected = HashSet::new();
        let now = std::time::Instant::now();
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("120"));
        selector.record_rate_limited("openai-primary", &members[0], &headers, now);
        assert_eq!(
            selector.select_member(
                "openai-primary",
                &members,
                &rejected,
                now + Duration::from_secs(29),
            ),
            Some(1)
        );
        assert_eq!(
            selector.select_member(
                "openai-primary",
                &members,
                &rejected,
                now + Duration::from_secs(31),
            ),
            Some(0)
        );

        // 新 generation 即使复用 member ID，也不继承旧快照的 cooldown。
        selector.record_rate_limited("openai-primary", &members[0], &headers, now);
        let next_store = credential_store(2);
        let next_members = next_store
            .upstream_pool(
                ProviderKind::OpenAi,
                "openai-primary",
                CredentialKind::ApiKey,
            )
            .unwrap();
        let rejected_peer = HashSet::from(["openai-primary#2".to_owned()]);
        assert_eq!(
            selector.select_member("openai-primary", &next_members, &rejected_peer, now),
            Some(0)
        );
        assert!(selector.has_available_member("openai-primary", &next_members, &rejected, now,));

        // 非法 Retry-After 使用 1 秒默认值，而不是永久封锁成员。
        let default_selector = CredentialHealth::default();
        let mut invalid_headers = HeaderMap::new();
        invalid_headers.insert("retry-after", HeaderValue::from_static("invalid"));
        default_selector.record_rate_limited("openai-primary", &members[0], &invalid_headers, now);
        let rejected_peer = HashSet::from(["openai-primary#2".to_owned()]);
        assert_eq!(
            default_selector.select_member(
                "openai-primary",
                &members,
                &rejected_peer,
                now + Duration::from_millis(500),
            ),
            None
        );
        assert_eq!(
            default_selector.select_member(
                "openai-primary",
                &members,
                &rejected_peer,
                now + Duration::from_secs(2),
            ),
            Some(0)
        );
    }
}
