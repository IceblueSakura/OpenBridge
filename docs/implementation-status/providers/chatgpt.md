# ChatGPT Provider 状态

## 当前实现

- Provider family 为 `chatgpt`，固定受信 origin 为 `https://chatgpt.com/backend-api/codex`，只接受
  `OAuth2BearerAccessToken`；OpenBridge 不读取本机 Codex auth/cache。
- Spark、GPT-5.5、GPT-5.6 Luna/Terra/Sol 各有固定 Responses-only Target，共用 `chatgpt-codex` OAuth pool。
- 每个 Target 要求上游 `stream:true`；下游非流式通过有界 Responses SSE takeover 生成 JSON。Chat 由受限
  Chat→Responses Bridge 提供。
- 固定 adapter 拥有 Models manifest、Responses path、SSE media/terminal、request identity header 和 stateless
  `store:false` envelope；它固定 `Accept: text/event-stream`、`originator: codex_cli_rs` 以及 headless Linux UA
  `codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown`，不从宿主 OS、terminal 或客户端输入派生。
  客户端不能覆盖 origin、credential 或这些 header。
- Spark 保持 text-only；其余固定 profile 声明 function tool、parallel tool call 与 structured output。输出 token limit
  字段和 Provider 私有 `include_reasoning` 在 egress 前拒绝；标准 `reasoning.encrypted_content` include 独立建模。
- 除 Spark 外，ChatGPT Codex Responses Target 向对应 Public Model 贡献保守的 Native inline 图片子集：每次最多一个
  JPEG/PNG/GIF/WebP data URL，Base64 payload 的单项及累计 encoded/decoded 上限分别为 20/15 MiB，只接受省略 detail。
  Public Model 仍按全部候选取交集，因此含未验证 OpenAI fallback 的 `gpt-5.6-sol` 当前不公开图片能力；Remote URL、
  file ID、显式 detail 与媒体 Bridge 保持关闭，部署级 request-body limit 另行生效。
- `gpt-5.6-sol` 还包含 OpenAI 后备 source，但 `SourceFirst` 使 Chat/Responses 都优先 ChatGPT；公共能力按全部固定候选交集公开。

## 所有权与确定性证据

- 注册与 wire 边界：[`src/providers/chatgpt/`](../../../src/providers/chatgpt/)。
- OAuth 生命周期：[`src/oauth2_credentials/`](../../../src/oauth2_credentials/)。
- `tests/oauth2_login_cli.rs`、`tests/startup_contract.rs` 与 `tests/upstream_credential_config.rs` 保护登录、bundle、启动与文件边界。
- `tests/forwarding_contract.rs` 和 `tests/bridge_conversion_contract.rs` 保护固定 envelope、OAuth lease、401 recovery、
  Native/Bridge 参数与 zero-egress 拒绝。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)经过本地 OpenBridge
验证五个含 ChatGPT source 的 Public Model 的 Chat/Responses × JSON/SSE × `none/high`。其中四个是 ChatGPT-only，
`gpt-5.6-sol` 还含 OpenAI source；该矩阵只走正常首选路径，不证明 Sol 的 OpenAI fallback。

GPT-5.6 Luna 的定向请求另观察到标准 `reasoning.encrypted_content` include 有无时都可能返回 opaque 内容；因此当前只把该值
解释为请求兼容，不承诺控制输出。真实 function tool、parallel、structured output、登录/refresh authority 与工具执行没有由该
文本矩阵证明。2026-08-18 的修复只确认 Hermes 图片请求此前在 OpenBridge preflight 因缺失 Provider contract
返回 `unsupported_model_capability`；本次没有执行真实图片 Provider 请求，因此不把声明的格式、大小或语义识别记为真实证据。

## 未证明边界

WebSocket、Batch、Embeddings、hosted/custom tool、MCP、真实图片输入、background、stateful response、完整 Agent loop、多账户轮换、
外部 SDK、负载和长期 refresh 稳定性均未证明。真实结果不构成其他账号、workspace entitlement 或未来 backend 的保证。

## 相关文档

- [ChatGPT OAuth2 功能状态](../features/chatgpt-oauth-startup.md)
- [能力探测](../capability-probing.md)
- [ChatGPT Models 参考](../../references/providers/chatgpt/models.md)
