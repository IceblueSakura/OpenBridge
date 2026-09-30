# 最小 Generation HTTP 网关

当前提供 `openbridge` binary 与可嵌入的 `gateway::Gateway`。它把现有语义库、双向 adapters、固定目标和实际 HTTP body 接通，不是旧服务的恢复或完整生产网关。决策见 [ADR 0009](architecture-v2/decisions/0009-minimal-http-text-gateway.md)，接口摘要见 [OpenAPI](openapi.json)。

## 启动

启动只读取代码显式声明的环境变量，**不自动读取 `.env`、旧 TOML 配置或其他应用认证缓存**。下表列通用启动控制；Provider 专用变量与模型/协议绑定从 [`src/gateway/bootstrap.rs`](../src/gateway/bootstrap.rs)及其引用的 catalog 查询，完整方法见 [AGENTS.md](../AGENTS.md#current-provider-model-and-compatibility-information)。

| 变量 | 含义 |
|---|---|
| `OPENBRIDGE_CLIENT_KEY` | 必填：单一入口 Bearer token，32–4096 个可打印 ASCII 非空白字符；应使用高熵随机值 |
| `OPENBRIDGE_BIND` | 可选：默认 `127.0.0.1:8080`；仅接受 literal loopback SocketAddr（也可 `[::1]:8080`） |
| `OPENBRIDGE_PROXY` | 可选：受信启动配置中的显式出站代理 URL；不继承 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` |
| `OPENBRIDGE_PROBE_DIAGNOSTICS` | 可选：操作者指定的新建私有 JSONL 文件；默认关闭，不覆盖已有文件，不记录 payload |

至少提供一个已注册 Provider 的 key；设置为空不等于禁用，而是配置错误。不输出凭据值，不生成默认入口密钥。模型绑定由 [`src/topology/catalog.rs`](../src/topology/catalog.rs)维护，未提供对应 key 的模型不在入口中开放。不要从变量命名规则猜测支持项，也不要通过打印环境或私有配置来确认启用情况。

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
- Chat 请求的 user/assistant、system/developer content 支持字符串或非空有序纯文本数组；单文本 part 编码规范化为字符串，多 part 保序。user content 另可包含有序 `image_url` 图片 parts，Responses 使用 `input_image`；只准入 URL/inline 图片输入，且 Public Model/Endpoint 必须声明可表示。详细边界见[图片输入 slice](architecture-v2/responses-text-profile.md#user-image-input)。工具结果支持字符串或有序纯文本数组，保留空数组和单/多 part，不拼接。function-only `allowed_tools` 使用 Chat 的嵌套 shell；实际目标支持仍须独立核对。
- 请求要求 JSON Content-Type，仅 UTF-8；不接受 Content-Encoding。严格 JSON 解析拒绝重复 key。先解析 envelope 中的 public model，绑定受信 task，再进行语义 decode。
- 每个 `(public model, client protocol)` 在启动时固定到 Route 中的一个 Endpoint；默认 bootstrap 使用相同 wire family。没有对应协议 entry 的模型返回 `model_not_found`；不从 Chat 绑定推导 Responses，也不隐式转协议。没有运行时候选重排、自动 retry/fallback 或业务 JSON 指定目标。
- operator 预算策略把**缺省输出上限**写入最终 IR，再计算 requirements、admission 与 lowering；显式上限超限则拒绝，不静默裁剪。Chat metadata/service-tier 和 logprobs/top_logprobs 经同一 typed IR/context 与 Endpoint gate，不因 codec 准入就自动扩大 catalog 模型能力。响应 reported facts 不从请求复制补齐。
- Provider URL、path、model 和 auth 都来自启动绑定。入站 headers 不透传，包括独立 `CodexHeaders` carrier；低层 Responses envelope 可读写的 `CustomSections` 也未接线，非空 sections 在请求准入时拒绝。上游非成功 HTTP 状态的诊断正文、认证状态细节、origin、凭据 locator 不回显；下游 `model` 为 public label。
- 未实现 `/v1/models`、状态资源、WebSocket、媒体资源服务/图片输出或 hosted-tool 执行。支持哪些语义仍取决于 public/endpoint 合同，不因 HTTP 路由存在而扩张。Chat 正文/refusal 概率按 owner 保真；静态 reported metadata 没有 Chat chunk 槽位，不能通过丢字段合成 SSE。

### 最小请求示例

使用入口 Bearer token（不是上游 API key）。下面的 `configured-public-model` 是占位符，不是已注册模型；按查询指南替换为目标实例已启用且准入对应协议的 public label。向真实模型发送仍会产生 Provider 调用，示例本身不授予调用权限：

```json
{"model":"configured-public-model","input":"Reply with exactly pong.","max_output_tokens":64}
```

将它发送到 `/v1/responses`；增加 `"stream":true` 即请求 SSE。对应 Chat 请求发送到 `/v1/chat/completions`：

```json
{"model":"configured-public-model","messages":[{"role":"user","content":"Reply with exactly pong."}],"max_completion_tokens":64}
```

图片输入示例（URL 仅为 synthetic 占位符，不表示上游可获取；实际测试可使用程序生成的 Base64 PNG）：

```json
{"model":"configured-public-model","input":[{"role":"user","content":[{"type":"input_text","text":"Describe the image."},{"type":"input_image","image_url":"https://example.test/synthetic.png"}]}],"max_output_tokens":64}
```

Chat 对应图片 part 为 `{"type":"image_url","image_url":{"url":"https://example.test/synthetic.png"}}`，前后的文本 part 用 `type:text`。不以图片作为普通字符串转发；file_id、工具图片结果与非 user 图片仍拒绝。库验证单资源 encoded/decoded 和总请求预算，HTTP 默认 256 KiB body 限制仍适用；不会为媒体自动放宽。URL 获取/尺寸/图像内容有效性由上游另行验证，网关不代为下载或转码。

厂商 routing policy 和 wire 扩展由[所选 adapter](../src/adapter/mod.rs)及 owning codec 维护；客户端不得覆盖上游路由、认证或可信 scope。示例不表示任一模型支持全部请求选项。

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

## 操作者诊断

显式 `OPENBRIDGE_PROBE_DIAGNOSTICS` 启用受控 probe 元数据，不是内容日志或生产观测系统。文件必须新建，父目录由操作者准备；Unix 权限 0600。启动时路径无效/已存在会拒绝启动。运行时采用容量 64 的 try-send 队列、每文件 1 MiB 上限；写失败/队列满会丢诊断，不改变业务响应或等待写入。Ctrl-C 后 best-effort 有界 drain；强杀或未完成 I/O 可使记录缺失，不能据缺失推断成功。

仅认证后的 POST 请求且唯一 `x-openbridge-probe-id` 符合 `<32位小写hex run-id>:<1–999999 attempt>` 时记录；无效/重复 ID 只禁用该请求诊断，不改变业务准入。它不进入 IR，不选择上游、不透传，不在响应中回显。

白名单仅含关联 ID、最后阶段/结果、上游 HTTP、0–86400 秒内的规范化 Retry-After、接收/已 handoff 字节与时间偏移。合法 HTTP-date 也规范化；未知或超范围值不保存。绝不记录正文、header 原文、URL、原始错误、credential locator、reasoning 或 opaque。静态 JSON 在 EOF finalize 时解析，其失败可出现在 `terminal` 阶段；`complete` 仍只表示 server transport handoff，不证明客户端收到。该通道不改变原有 429/502 映射，不把 Retry-After 透传下游，不触发 retry/fallback。

嵌入方可在共享 Gateway 前调用 `with_probe_diagnostics(path)`，结束时 `flush_probe_diagnostics().await` 做有界 drain。run/attempt 及 SDK/pi 账本的归属见 [probe 指南](probes.md)。

## 嵌入与验证

`Gateway::new` 接受已编译 topology、显式 entries、凭据绑定和资源限制；不读取环境。`router()` 用于嵌入，调用方必须维持 listener 安全边界；`serve()` 自身检查 loopback 并连接 shutdown；丢弃 serve future 也会关闭其上游 workers，避免只能等待请求 deadline。显式跨协议 entry 仍走同一 IR/投影，无法表示的内容拒绝，不能作为任意转换保证。

- `src/gateway/` 的 owner tests 验证认证、预算、scope、body 背压/取消/deadline/终态。
- `src/transport/http_tests.rs` 验证实际 loopback HTTP 的 no-redirect/no-retry 和 timeout。
- `tests/gateway.rs` 使用真实 Router 与 synthetic HTTP Provider；另外通过隔离环境启动 binary，拒绝代理捕获器阻止任何意外外部请求。
- `tests/sdk/gateway.rs` 与 `gateway_text_loop.py` 让固定 SDK 经同一 Router/Provider 完成双协议 JSON/SSE 工具与 reasoning 续轮；与旧的纯 fixture SDK gates 分开。

运行方式见[开发指南](development.md)。需要外部兼容性结论时，按 [probe 指南](probes.md)取得授权并验证选定实例/目标，不在本文保存结果。库级测试不能替代 HTTP 服务验收，局部通过也不证明部署、长稳、缓存收益或更广 Agent 行为。
