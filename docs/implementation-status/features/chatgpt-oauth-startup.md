# 功能：ChatGPT OAuth2 生命周期与 Responses 数据面

## 当前行为

- `openbridge-auth login chatgpt` 使用固定 device interaction 与 authorization-code + PKCE，校验 bundle 后事务写入
  OpenBridge-owned auth file；服务不读取本机 Codex auth/cache。
- login CLI 可以把成功取得的完整 bundle 原子写入尚不存在的 auth file；它不会先创建空白占位。主服务启动要求该文件已存在且
  严格通过 Provider/context、token type、expiry 与完整性校验，缺失、空白或损坏文件都会阻止启动。
- Manager 在进程 gate 和 advisory file lock 内 guarded reload/refresh，原子发布 credential generation；请求只借用短生命周期、
  account-bound lease。首个预提交 401 guarded recover/replay 一次，重复 401 标记 reauth required。
- Spark、GPT-5.5、GPT-5.6 Luna/Terra/Sol 使用固定 Responses-only Target 与共享 OAuth pool；Chat 由受限 Bridge 提供。
- Adapter 固定 Codex origin、Models manifest、Responses path、`stream:true`、`store:false`、input envelope、SSE terminal/media 和
  request identity。具体固定 UA/header 见 [ChatGPT Provider 状态](../providers/chatgpt.md)。
- 下游非流式 Chat/Responses 通过有界 SSE takeover；输出 token limit 与 Provider 私有 `include_reasoning` 拒绝，标准
  `reasoning.encrypted_content` 独立按 request compatibility 处理。
- 默认 instructions 由通用 planning 负责：客户端显式值优先，否则使用项目默认；adapter 不再覆盖 instruction context。

## 所有权

OAuth 位于 [`src/oauth2_credentials/`](../../../src/oauth2_credentials/)，ChatGPT registration/wire 位于
[`src/providers/chatgpt/`](../../../src/providers/chatgpt/)，401 recovery 位于 `src/ingress/forwarding.rs`。

## 确定性与真实证据

`tests/oauth2_login_cli.rs`、`tests/startup_contract.rs`、`tests/upstream_credential_config.rs` 与 manager 单元测试保护登录、文件、
refresh/single-flight/generation；`tests/forwarding_contract.rs` 与 Bridge tests 保护固定 envelope、OAuth lease、401、参数、JSON/SSE
和 zero egress。

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)记录五个含 ChatGPT source 的 Public
Model 正常首选文本路径；其中四个为 ChatGPT-only，`gpt-5.6-sol` 还含 OpenAI source。GPT-5.6 Luna 的定向 include 对照说明
opaque reasoning 可能在有无 include 时都出现，因此不把 include
解释为输出开关。

## 未证明范围

真实登录/refresh authority、真实 function/parallel/structured-output、其他账号/workspace entitlement、WebSocket、Batch、Embeddings、
hosted/custom tool、多模态、background/state、完整 Agent loop、负载和长期 refresh 稳定性未证明。

## 相关文档

- [OAuth credential lifecycle 需求](../../functional-requirements/configuration-credentials/upstream-oauth-credential-lifecycle.md)
- [ChatGPT Provider 状态](../providers/chatgpt.md)
- [能力探测](../capability-probing.md)
