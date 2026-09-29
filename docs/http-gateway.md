# 最小文本 HTTP 网关

当前提供 `openbridge` binary 与可嵌入的 `gateway::Gateway`。它把现有语义库、双向 adapters、固定目标和实际 HTTP body 接通，不是旧服务的恢复或完整生产网关。决策见 [ADR 0009](architecture-v2/decisions/0009-minimal-http-text-gateway.md)，接口摘要见 [OpenAPI](openapi.json)。

## 启动

启动只读取下列环境变量，**不自动读取 `.env`、旧 TOML 配置或其他应用认证缓存**。

| 变量 | 含义 |
|---|---|
| `OPENBRIDGE_CLIENT_KEY` | 必填：单一入口 Bearer token，32–4096 个可打印 ASCII 非空白字符；应使用高熵随机值 |
| `OPENBRIDGE_DEEPSEEK_API_KEY` | 可选：启用固定 catalog 中的 DeepSeek 模型 |
| `OPENBRIDGE_XIAOMI_API_KEY` | 可选：启用固定 catalog 中的 Xiaomi 模型 |
| `OPENBRIDGE_OPENROUTER_API_KEY` | 可选：启用 `gpt-6-luna`，固定上游 ID 为 `openai/gpt-6-luna` |
| `OPENBRIDGE_BIND` | 可选：默认 `127.0.0.1:8080`；仅接受 literal loopback SocketAddr（也可 `[::1]:8080`） |
| `OPENBRIDGE_PROXY` | 可选：受信启动配置中的显式出站代理 URL；不继承 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` |

至少提供一个 Provider key；设置为空不等于禁用，而是配置错误。不输出凭据值，不生成默认入口密钥。模型绑定由 [`src/topology/catalog.rs`](../src/topology/catalog.rs)维护；未提供对应 key 的模型不在入口中开放。此前的 MiMo Flash 测试绑定不会自动进入服务。

在安全地提供上述环境变量后启动：

```sh
cargo run --locked --offline --bin openbridge
```

只有启动不会产生模型生成请求；向有效模型提交请求可能产生真实费用。Ctrl-C 发起 graceful shutdown，取消在途上游并拒绝新的业务请求。这个入口不是部署指令；默认验证仅使用 synthetic keys 和 loopback Provider。

## HTTP 合同

| 方法 / 路径 | 请求与交付 |
|---|---|
| `POST /v1/chat/completions` | [单候选 Chat profile](architecture-v2/chat-text-profile.md)；JSON 或 SSE |
| `POST /v1/responses` | [无状态 Responses text profile](architecture-v2/responses-text-profile.md)；JSON 或 SSE |

- 所有路由先检查唯一的 `Authorization: Bearer …`，认证通过后才进行应用层 body 收集。其他认证 header 不替代该字段，重复 Authorization 拒绝。
- 请求要求 JSON Content-Type，仅 UTF-8；不接受 Content-Encoding。严格 JSON 解析拒绝重复 key。先解析 envelope 中的 public model，绑定受信 task，再进行语义 decode。
- 每个 `(public model, client protocol)` 在启动时固定到 Route 中的一个 Endpoint；默认 bootstrap 使用相同 wire family。没有运行时候选重排、自动 retry/fallback 或业务 JSON 指定目标。
- operator 预算策略把**缺省输出上限**写入最终 IR，再计算 requirements、admission 与 lowering；显式上限超限则拒绝，不静默裁剪。响应 reported facts 不从请求复制补齐。
- Provider URL、path、model 和 auth 都来自启动绑定。入站 headers 不透传，上游非成功 HTTP 状态的诊断正文、认证状态细节、origin、凭据 locator 不回显；下游 `model` 为 public label。
- 未实现 `/v1/models`、状态资源、WebSocket、媒体或 hosted-tool 执行。支持哪些语义仍取决于 public/endpoint 合同，不因 HTTP 路由存在而扩张。

### 最小请求示例

使用入口 Bearer token（不是上游 API key）。下面是 synthetic 请求 body；若向已启用的真实模型发送，仍会产生真实 Provider 调用，示例本身不授予调用权限：

```json
{"model":"deepseek-flash","input":"Reply with exactly pong.","max_output_tokens":64}
```

将它发送到 `/v1/responses`；增加 `"stream":true` 即请求 SSE。对应 Chat 请求发送到 `/v1/chat/completions`：

```json
{"model":"deepseek-flash","messages":[{"role":"user","content":"Reply with exactly pong."}],"max_completion_tokens":64}
```

OpenRouter 启用后，上述两个示例可将 `model` 改为 `gpt-6-luna`；只接入普通版本，不是 Pro/batch。adapter 固定发送 `provider.require_parameters=true`，客户端不得传入 `provider` 覆盖 routing。其文本准入、reasoning/费用字段映射和未支持的 wire 分支见 [OpenRouter text adapter](architecture-v2/openrouter-text-profile.md)。`openai-responses-v1` 的 Chat summary/encrypted details 已按 owner/origin 约束接入；其他格式仍拒绝，实际验收范围见该页关联的证据。

模型必须已通过启动凭据启用；其他字段按关联 text profile 准入。固定 SDK 使用 `base_url` 指向本机 `/v1`，`api_key` 使用入口 token，不把上游 key 交给客户端。

### 默认资源边界

| 边界 | 默认 |
|---|---:|
| 应用层在途请求 | 16；满时立即 429，不排无限队列 |
| 请求 body / 收集超时 | 256 KiB / 10 秒 |
| 上游响应 / 下游编码预算 | 8 MiB，另外受 Endpoint/codec 自身预算约束 |
| 单 SSE frame / 事件数 | 1 MiB / 65,536 |
| 上游请求到下游 body handoff 的绝对 deadline | 120 秒，与 Endpoint timeout 取更短者 |
| 缺省 / 最大允许请求输出 tokens | 1024 / 16384 |

这些是应用层边界，不是对模型实际计费、全部 HTTP 连接资源或生产抗负载能力的保证。嵌入方可通过 `Limits` 显式调整有效范围。

### 错误与流式边界

应用层错误采用 `{"error":{"message":…,"type":…,"param":null,"code":…}}`；不把原始 parser/Provider 错误字符串写入响应。HTTP framing 层拒绝由 HTTP server 处理。

| HTTP | 应用层原因示例 |
|---|---|
| 400 | 无效/未准入语义、重复 JSON key、`output_limit_exceeded` |
| 401 | 缺失、无效或重复入口认证；带 `WWW-Authenticate: Bearer` |
| 404 / 405 | 未开放的模型或路由 / 方法不支持 |
| 408 / 413 / 415 | 请求收集超时 / body 超限 / 媒体类型或编码不支持 |
| 429 | `gateway_busy` 或上游 rate limit |
| 502 / 504 | 上游状态、协议、投影或预算失败 / 上游交付超时 |
| 503 / 500 | 服务关闭中 / 本地运行故障 |

首个可交付 frame 产生前失败可返回 JSON 错误。HTTP response 已交出后不能更改状态：late error、取消、超时或缺失/错误终态会中止 body，不合成成功 `response.completed` / `[DONE]`，也不重试。已准入的模型非成功语义终态（如 incomplete）与 transport 错误不同，仍按语义合同交付。

SSE 不收完整流再回放。每次最多消费一个上游 frame；下游 frame 在 HTTP body handoff 时确认，未确认不推进后续语义处理。仅编码或排队不算 commit。严格上游 EOF 后才释放终态；完成全部 handoff 后才完成 producer。这是服务 transport 边界，不声称已收到客户端/TCP acknowledgement。消费者不 poll body 时，deadline 仍能释放上游；drop/shutdown 同样取消资源。

## 嵌入与验证

`Gateway::new` 接受已编译 topology、显式 entries、凭据绑定和资源限制；不读取环境。`router()` 用于嵌入，调用方必须维持 listener 安全边界；`serve()` 自身检查 loopback 并连接 shutdown；丢弃 serve future 也会关闭其上游 workers，避免只能等待请求 deadline。显式跨协议 entry 仍走同一 IR/投影，无法表示的内容拒绝，不能作为任意转换保证。

- `src/gateway/` 的 owner tests 验证认证、预算、scope、body 背压/取消/deadline/终态。
- `src/transport/http_tests.rs` 验证实际 loopback HTTP 的 no-redirect/no-retry 和 timeout。
- `tests/gateway.rs` 使用真实 Router 与 synthetic HTTP Provider；另外通过隔离环境启动 binary，拒绝代理捕获器阻止任何意外外部请求。
- `tests/sdk/gateway.rs` 与 `gateway_text_loop.py` 让固定 SDK 经同一 Router/Provider 完成双协议 JSON/SSE 工具与 reasoning 续轮；与旧的纯 fixture SDK gates 分开。

运行方式见[开发指南](development.md)。[GPT-6 Luna 受控验收](implementation-status/evidence/2026-09-29-openrouter-luna-acceptance.md)包含固定 SDK 经实际 binary 的双协议 JSON/SSE 文本与工具续轮；只证明其指定场景。部署、长稳压测、缓存收益、多租户和更广 Agent 行为仍未验收。旧 live 库级证据不能替代新 HTTP 服务的外部验收。
