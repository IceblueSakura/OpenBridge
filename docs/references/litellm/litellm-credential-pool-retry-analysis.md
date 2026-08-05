# LiteLLM deployment retry 与 cooldown 调研

## 证据范围

- 固定快照：`BerriAI/litellm` commit `de706a35a6f1e9cb8c3cb527271df0b76a69f410`，2026-08-02
- 阅读入口：Router 与 reliability 文档
- 本文只记录 LiteLLM deployment selection、retry、fallback 和 cooldown 的外部行为。

## 观察事实

- Router 可在同一个 model group 的多个 deployment 间做选择。
- cooldown 作用于单个 deployment；其他同组 deployment 仍可使用。
- 429 会触发 deployment cooldown；retry 次数与 cooldown 独立配置，并可按错误类别设置。
- Router 还可结合 Redis、预算、团队、virtual key 和动态配置，这些是 LiteLLM Proxy 管理面的一部分。

## 适用边界

- “最小可替换资源”由 LiteLLM 的 deployment 模型定义，不自动等价于 Provider、credential 或 endpoint。
- model group 中存在一个可用 deployment，不代表所有 deployment 的 capability、identity 或 state 可互换。
- 分布式状态依赖其 Proxy/Redis 配置，不能只从本地 Router 行为推断。

## 一手资料

- [LiteLLM routing](https://docs.litellm.ai/docs/routing)
- [LiteLLM reliability](https://docs.litellm.ai/docs/proxy/reliability)

