# Images 剩余执行证明与 legacy 收口

本页只拥有阶段 4–7 尚未关闭的执行一致性、测试证明与 legacy 清理。

## Images 剩余执行证明

当前 Images 纵向路径尚未满足原定的全部执行与证据边界，收口前需要单独 current focus：

- 明确 `TransportError::Timeout` 的稳定 504 `upstream_timeout` 合同，并用 operation-specific test 保护；
- 决定单次、不可重放的 Images attempt 如何进入共享 execution coordinator，或基于证据修订该共同路径不变量；
- 覆盖 response body 超限、读取失败、提前 EOF、下游取消、attempt accounting 与 commit 后行为；
- 覆盖 operation label、image usage units 以及 prompt、图片 URL 和上游 body 不进入普通 telemetry；
- 为 `(operation, task)` 错误绑定、Provider ceiling/Target narrowing、多候选公共交集和 Bridge/resource fail-closed 建立专项证据；
- 为 Images profile 的 subset、intersection、candidate-order 和 public projection 代数规则补齐确定性 law tests。

本页不判断当前 Images 实现状态；可用路径、源码 owner 与已执行证据见[Native Images 实施现状](../../implementation-status/features/native-images-generation.md)，并须在进入对应 current focus 时从 live source 重新核验。上述缺口关闭前不能宣称原设计包的完整纵向证明已经完成。

## Legacy 清理

完成上述证明后执行一次有清单的全仓库删除审查：

- old capability/module/type names；
- compatibility conversion 和 unused alias；
- operation-only API key 与固定 private operation fields；
- registration media mutation；
- duplicate test builders、orphan fixtures 和 stale links。

不得以 TODO、feature flag 或 dead branch 保留旧路径。没有发现具体残留时也应记录检索范围，而不能只凭当前编译通过宣称完成。

## 退出门

- Images 剩余 focused tests 全绿；
- profile algebra 与 Provider/Target conformance 证据完整；
- full Rust、OpenAPI、link 和 `git diff --check` 全绿，涉及 corpus 时追加 corpus baseline；
- requirements、OpenAPI 与 implementation status 和 live source 一致；
- legacy 删除审查有可复核结果；
- `current-focus.md` 恢复为空。

## 后续扩展

每个后续 operation 仍使用单独 current focus：wire/limits/errors → RED fixture → operation/profile → registry interface → pipeline/adapter →
Provider evidence。框架存在不授权批量打开未验证 capability。
