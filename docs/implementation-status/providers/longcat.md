# LongCat 2.0 Provider 状态

## 当前注册

- 固定 origin 为 `https://api.longcat.chat`，使用 `longcat-primary` API-key pool。
- `LongCat-2.0` 保留 Chat/Responses Native 与两个显式 Bridge 候选；reasoning output 为 `PlainText`。
- canonical reasoning 是二态 `none/high`，两个 Public Model interface 对固定候选取交集后也公开 `none/high`。
- Chat egress 把标准 `reasoning_effort:none/high` 固定转换为官方 `thinking.type=disabled/enabled`；Responses 保留标准
  `reasoning.effort`。

## 证据

- 官方 Chat 文档明确声明 `thinking.type` 的 enabled/disabled 二态；官方 Codex/CC Switch 配置确认 Native Responses 与
  `model_reasoning_effort=high`。外部证据见 [LongCat API 调研](../../references/providers/longcat/api.md)。
- 2026-08-08 真实下游 E2E 已覆盖 Chat/Responses × JSON/SSE × high，共 4 个单元，全部 HTTP 200、终态完整且 reasoning 非空。
- 当前确定性测试覆盖 canonical level、Public Model 交集、none/high planning 与 Chat 官方 wire 转换。

## 未验证边界

本轮没有使用真实 LongCat key 复测 none，也没有执行外部 SDK、负载或长期运行。确定性测试和官方参数说明不等同于当前账号的
真实 Provider 验收；Native Responses 的完整 reasoning 枚举仍未由官方 API 参考列出。
