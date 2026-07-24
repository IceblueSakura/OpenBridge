# M00 产品边界

## 目标

OpenBridge 是单用户、单服务部署的多 Provider Agent API 聚合代理。服务所有者集中管理 Provider、credential、deployment、模型 alias 和路由；Codex、Hermes Agent 等客户端只访问 OpenBridge。

## 功能优先级

### P0：可用且安全的原生代理

- Chat Completions 与 Responses 的 JSON/SSE；
- public model alias；
- Codex Responses HTTP/SSE 和 Hermes Chat；
- 原生字段保真、SSE、取消、错误和首输出前 fallback；
- 静态入站认证、secret isolation、origin allowlist 和资源上限；
- 固定版本目标客户端与真实/脱敏 Provider corpus。

### P1：多 Provider 协议桥核心

- 至少两个 Provider Family；
- deployment 级有限 retry、被动 cooldown、fallback 和安全错误传播；
- Responses → Chat 和 Chat → Responses 最小工具循环；
- 非 OpenAI wire dialect 的异构 Provider 验证；
- Provider conformance suite；
- 可发布、安全、可回滚的核心版本。

### P2：核心后增强

- usage/成本记录；
- 最小 cooldown 之上的高级健康观测与自适应路由；
- Provider-hosted tool facade；
- 本地/MCP Tool Bridge；
- 可选 OAuth；
- 简单管理 UI。

## 非目标

- 多租户、团队、ACL、面向下游用户/key 的配额、计费和合规控制面；
- 多账号 credential pool 和账号轮换；
- OpenAI 全资源 API、Realtime、Files、Conversations 和管理 API；
- 首版 Responses WebSocket；
- 无损 Chat ↔ Responses 承诺；
- 客户端动态指定上游 URL、credential、认证 header 或转换脚本；
- 核心代理执行任意通用 function tool。

## 详细资料

- [核心需求](../requirements/proxy-requirements.md)
- [Provider 韧性需求](../requirements/provider-resilience.md)
- [目标架构](../architecture/architecture-and-roadmap.md)
- [Hosted tool 增强需求](../requirements/hosted-tools-mcp.md)
