//! 单个下游请求内的有限上游 attempt、候选保留与退避策略。
//!
//! 本模块只管理请求级计数和时间边界，不选择 Route、Provider 或错误类别。调用方必须先
//! 完成 RoutePlan 与 adapter 分类；固定硬上限保证任何请求都不能形成无限上游循环。

use std::time::Duration;

const MAX_REQUEST_ATTEMPTS: usize = 6;
const MAX_CANDIDATE_ATTEMPTS: usize = 2;
const INITIAL_BACKOFF: Duration = Duration::from_millis(50);
const MAX_BACKOFF: Duration = Duration::from_millis(500);

/// 一个 retryable failure 之后允许采取的下一步。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AttemptStep {
    /// 退避后重试当前候选。
    RetryCandidate,
    /// 退避后进入下一个已规划候选。
    NextCandidate,
    /// 总预算或候选已耗尽，返回当前失败。
    Finish,
}

/// 单个下游请求共享的 attempt 预算和 capped exponential backoff 状态。
pub(super) struct AttemptManager {
    attempts_started: usize,
    candidate_attempts: usize,
    next_backoff: Duration,
}

impl AttemptManager {
    /// 创建尚未发起上游调用的请求级管理器。
    pub(super) fn new() -> Self {
        Self {
            attempts_started: 0,
            candidate_attempts: 0,
            next_backoff: INITIAL_BACKOFF,
        }
    }

    /// 开始一个新候选并清空其局部 attempt 计数。
    pub(super) fn begin_candidate(&mut self) {
        self.candidate_attempts = 0;
    }

    /// 消耗一次请求级和候选级 attempt 预算。
    pub(super) fn start_attempt(&mut self) -> bool {
        // 拒绝超过请求级硬上限的调用。
        if self.attempts_started >= MAX_REQUEST_ATTEMPTS {
            return false;
        }

        // 同时记录请求级与当前候选的 attempt。
        self.attempts_started += 1;
        self.candidate_attempts += 1;
        true
    }

    /// 根据剩余未尝试候选数选择 retry、fallback 或结束。
    ///
    /// 当前候选只有在预算仍能容纳剩余候选时才可重试；无论 Route 数量如何，请求级硬
    /// 上限始终优先，避免配置规模放大单请求上游调用次数。
    pub(super) fn next_step(&self, untried_candidates: usize) -> AttemptStep {
        // 判断当前重试是否仍能为预算范围内的未尝试候选保留机会。
        let reserves_untried_candidates =
            self.attempts_started + untried_candidates < MAX_REQUEST_ATTEMPTS;

        // 优先有限重试当前候选，再选择 fallback，最终收敛为返回当前失败。
        if self.candidate_attempts < MAX_CANDIDATE_ATTEMPTS
            && reserves_untried_candidates
            && self.attempts_started < MAX_REQUEST_ATTEMPTS
        {
            AttemptStep::RetryCandidate
        } else if untried_candidates > 0 && self.attempts_started < MAX_REQUEST_ATTEMPTS {
            AttemptStep::NextCandidate
        } else {
            AttemptStep::Finish
        }
    }

    /// 等待下一次 attempt，并推进 capped exponential backoff。
    ///
    /// 下游任务被取消时，`sleep` future 与整个 manager 一同释放，不会在后台唤醒并继续请求。
    pub(super) async fn wait_before_next_attempt(&mut self) {
        // 固化本次延迟并计算下一档 capped exponential backoff。
        let delay = self.take_backoff_delay();

        // 等待可取消的 Tokio timer 后再允许下一次上游调用。
        tokio::time::sleep(delay).await;
    }

    fn take_backoff_delay(&mut self) -> Duration {
        let delay = self.next_backoff;
        self.next_backoff = self.next_backoff.saturating_mul(2).min(MAX_BACKOFF);
        delay
    }
}

#[cfg(test)]
mod tests {
    use super::{AttemptManager, AttemptStep, INITIAL_BACKOFF, MAX_BACKOFF, MAX_REQUEST_ATTEMPTS};

    #[test]
    fn request_budget_reserves_untried_candidates_and_has_a_hard_limit() {
        let mut attempts = AttemptManager::new();
        attempts.begin_candidate();

        // 验证候选局部重试不会挤占剩余候选的保留机会。
        assert!(attempts.start_attempt());
        assert_eq!(attempts.next_step(3), AttemptStep::RetryCandidate);
        assert!(attempts.start_attempt());
        assert_eq!(attempts.next_step(3), AttemptStep::NextCandidate);

        // 消耗剩余请求预算并验证硬上限拒绝额外 attempt。
        for _ in 2..MAX_REQUEST_ATTEMPTS {
            attempts.begin_candidate();
            assert!(attempts.start_attempt());
        }
        assert!(!attempts.start_attempt());
        assert_eq!(attempts.next_step(1), AttemptStep::Finish);
    }

    #[test]
    fn backoff_doubles_and_stops_at_the_cap() {
        let mut attempts = AttemptManager::new();

        // 验证延迟按二倍增长，并在 500 ms 上限保持稳定。
        assert_eq!(attempts.take_backoff_delay(), INITIAL_BACKOFF);
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(2)
        );
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(4)
        );
        assert_eq!(
            attempts.take_backoff_delay(),
            INITIAL_BACKOFF.saturating_mul(8)
        );
        assert_eq!(attempts.take_backoff_delay(), MAX_BACKOFF);
        assert_eq!(attempts.take_backoff_delay(), MAX_BACKOFF);
    }
}
