# 最小 Generation HTTP 网关

`openbridge` binary 与可嵌入的 `gateway::Gateway` 将共享 IR、双向 adapters、固定目标和实际 HTTP body 接通，不是完整标准实现或生产网关。HTTP 决策归 [ADR 0009](architecture-v2/decisions/0009-minimal-http-text-gateway.md)，公共接口摘要归 [OpenAPI](openapi.json)，模块接线归[架构](architecture.md)。

## 启动

启动只读取操作者指定的私有 JSON 配置与凭据目录，不读取环境 key/账户 alias、`.env`、旧 TOML 或第三方 auth cache。默认入口配置为目录内的 `gateway.json`，可用 `--config` 指定独立路径：

```sh
cargo run --locked --offline --bin openbridge -- --credentials-dir /path/to/private-store
cargo run --locked --offline --bin openbridge -- \
  --credentials-dir /path/to/private-store --config /path/to/private/gateway.json
```

配置结构、synthetic 示例、API key/OAuth 与 pool 操作由[凭据指南](credentials.md#gateway-access-绑定)维护；精确解析归 [bootstrap](../src/gateway/bootstrap.rs)。入口 `client_key` 不等于上游 key，listener 只允许 literal loopback，出站 proxy 必须显式受信配置，不继承环境代理。不得打印配置正文。

Pool 按编译 binding 启用候选；只有账户或 key、没有 pool，不激活推理。凭据绑定变化与并发管理的规则见[凭据指南](credentials.md)；本地可借用不证明上游授权、模型资格或额度。

仅启动不产生模型生成请求。真实登录、推理与付费测试需另行授权；默认验收只用 synthetic keys 和 loopback Provider。Ctrl-C 发起 graceful shutdown，取消在途上游并拒绝新业务请求，不证明 Provider 停算或停止计费。

## HTTP 合同

| 方法 / 路径 | 请求与交付 |
|---|---|
| `POST /v1/chat/completions` | [单候选 Chat profile](architecture-v2/chat-text-profile.md)；JSON 或 SSE |
| `POST /v1/responses` | [无状态 Responses profile](architecture-v2/responses-text-profile.md)；JSON 或 SSE |

- 唯一的 `Authorization: Bearer …` 在应用层 body 收集前校验；重复或错误认证拒绝，其他 header 不替代它。请求使用 UTF-8 JSON Content-Type，不接受 Content-Encoding；严格 JSON 拒绝重复 key，先绑定 public model/task 再 decode 语义。
- `model` 仅接受已激活的 public label，不带 `provider/` 前缀。顶层 `provider` 字段即使为 null 也拒绝。目标 URL/path、上游 model、auth、adapter 与 scope 都来自受信绑定；入站 headers 不透传。
- 每个 `(public model, client protocol)` 显式激活编译 Route 成员。保持 Route 和 pool 顺序，从同一最终 IR 独立预检每个固定 `(endpoint, credential)`；无兼容成员在 I/O 前失败。注册、Chat 激活与 Responses 激活不互相推定，查询方法见 [AGENTS](../AGENTS.md#current-provider-model-and-compatibility-information)。
- operator 缺省输出上限先写入最终 IR，再派生 requirements/admission/lowering；显式超限拒绝，不静默裁剪。响应 reported facts 不从请求补齐。
- 文本数组、工具选择/结果、概率、Schema 与 reported context 的精确接受/拒绝由上述 profiles 和 owning code 维护，不因路由存在而扩大 Public Model/Endpoint 合同。跨协议不可表示时明确拒绝，不丢字段换取成功。
- user 与 Responses 工具结果的 URL/inline 图片有独立准入，见[图片输入](architecture-v2/responses-text-profile.md#user-image-input)与[工具图片结果](architecture-v2/responses-text-profile.md#tool-image-results)。不下载、转码或自动放宽 body 预算；file ID、Chat 工具图片、图片输出与资源服务不因此启用。
- 标准 Responses 无 Chat message-call 归属 carrier：显式分组 history 在上游 I/O 前拒绝；无法交付的静态输出返回 502，SSE 在首次 attached-call 事件失败，已发布前缀只能中止，不能伪造成功或前移。独立 calls 与同协议分组仍按[分组合同](architecture-v2/responses-text-profile.md#message-owners-and-cross-protocol-grouping)处理。
- 标准 identity/cache hints 与客户端 `session_id` body 扩展使用各自声明的目标投影，不互相派生，不透传 session headers。`session_id` 不提供网关会话或粘性路由；精确 carrier 归 [adapter request](../src/adapter/request.rs)与[cache projection](../src/protocol/cache.rs)。未声明 carrier 的 advisory cache hint 可按合同省略，行为控制与 identity/session 要求不能随之静默丢弃。
- [Continuation](architecture-v2/responses-text-profile.md#response-outcome-and-continuation)库视图不增加 HTTP 字段、执行就绪证明或自动 Agent loop。低层 CustomSections/CodexHeaders 也不等于 HTTP 接线；仅开放表中路由，不提供 `/v1/models`；状态资源、WebSocket、hosted-tool/program 执行等[缺口](implementation-status/generation.md)仍独立。

### 最小请求示例

`configured-public-model` 是占位符，不是已注册模型。先按查询指南替换为目标实例已激活、准入相应协议的 public label；示例不授予真实调用权限。

Responses 请求发送到 `/v1/responses`，增加 `"stream":true` 请求 SSE：

```json
{"model":"configured-public-model","input":"Reply with exactly pong.","max_output_tokens":64}
```

Chat 请求发送到 `/v1/chat/completions`：

```json
{"model":"configured-public-model","messages":[{"role":"user","content":"Reply with exactly pong."}],"max_completion_tokens":64}
```

图片示例仅使用 synthetic URL，不表示上游可获取或模型已准入：

```json
{"model":"configured-public-model","input":[{"role":"user","content":[{"type":"input_text","text":"Describe the image."},{"type":"input_image","image_url":"https://example.test/synthetic.png"}]}],"max_output_tokens":64}
```

Chat 图片 part 使用 `{"type":"image_url","image_url":{"url":"https://example.test/synthetic.png"}}`。实际图片与资源限制仍由目标合同检查，不把媒体当普通字符串。

固定 SDK 的 `base_url` 指向本机 `/v1`，`api_key` 使用入口 token；不把上游 key 交给客户端。

### 默认资源边界

默认值与可嵌入调整范围只由 [`Limits`](../src/gateway/config.rs)维护：应用层并发、请求收集、上游/下游 bytes、单 SSE frame/事件数、输出 tokens 与绝对 exchange deadline 分别有界，且仍受 Endpoint/codec 预算约束。满载拒绝，不排无限队列；更紧的 Endpoint timeout 优先。媒体输入不自动放宽限制，背压不暂停 deadline。

这些是应用层边界，不是全部 HTTP 连接、实际计费或生产抗负载保证。

### 错误与流式边界

应用错误使用 `{"error":{"message":…,"type":…,"param":null,"code":…}}`；HTTP framing 拒绝归 server。原始 parser/Provider 错误、上游正文、auth 状态细节、origin 与 credential locator 不回显；下游 model 保持 public label。

| HTTP | 应用层原因示例 |
|---|---|
| 400 | 无效/未准入语义、重复 JSON key、输出上限超限 |
| 401 | 缺失、无效或重复入口认证；带 `WWW-Authenticate: Bearer` |
| 404 / 405 | 未开放模型或路由 / 方法不支持 |
| 408 / 413 / 415 | 请求收集超时 / body 超限 / 媒体类型或编码不支持 |
| 429 | 应用并发满或上游 rate limit |
| 502 / 504 | 上游状态、协议、投影或预算失败 / 交付超时 |
| 503 / 500 | 服务关闭或绑定凭据不可用 / 本地运行故障 |

跨 Endpoint fallback 须显式 Route 策略，同 Endpoint 换凭据须显式 pool 策略；全部候选共用一次 permit、尝试上限与绝对 deadline，无同成员重试/竞速/运行时改序。允许失败与身份边界归 [ADR 0010](architecture-v2/decisions/0010-canonical-model-fixed-fallback.md)和[凭据 fallback](credentials.md#同-provider-fallback)：auth/权限、重定向、协议/投影、安全故障不前移，未知 scope 的 429 不授权换凭据。多成员入口仍拒绝没有 affinity carrier 的 opaque replay/加密输出请求。

首次下游 frame 发布即保守冻结前移，body handoff 才确认 commit；HTTP response 已交出后不能改状态。Late error、取消、超时、错误/缺失终态中止 body，不合成成功 `response.completed` / `[DONE]`。模型 incomplete 等合法非成功语义终态与 transport 失败分开，不作为 fallback 理由。

上游强制 SSE 与下游交付独立：JSON 有界聚合至验证终态及严格 EOF，SSE 增量交付。只有具名 profile 可接受缺失 Content-Type 的固定 SSE，显式冲突仍拒绝。每步至多消费一个 frame，未 handoff 不推进后续语义处理；严格 EOF 后释放终态，最终 handoff 后完成。编码/排队不是 commit，server handoff 也不是客户端/TCP acknowledgement。Drop、shutdown 或消费者不 poll 时，取消/deadline 仍释放上游资源。

## 操作者诊断

路由定位使用 [Public Model → Route → Endpoint 查询流程](../AGENTS.md#current-provider-model-and-compatibility-information)，不按名称前缀猜测，也不读取/打印私有配置。静态注册与运行实例激活分开报告。

入口配置的 `diagnostics` 显式启用本地 probe 元数据，不是内容日志或生产观测。父目录由操作者准备，文件必须新建；精确权限、队列、文件预算与白名单归 [diagnostics owner](../src/gateway/diagnostics.rs)。无效路径/已存在文件拒绝启动，运行时写失败或队列满丢诊断，不改变业务响应。结束时 best-effort 有界 drain；缺少记录只能记为未知。

仅认证后的 POST 且唯一 `x-openbridge-probe-id` 符合 owner 语法才关联；无效/重复 ID 禁用观察但不改变请求准入。ID 不进入 IR、不选择上游、不透传或回显。诊断只保存封闭阶段/结果、HTTP、规范化 Retry-After、字节与时间及候选观察，不保存正文、header 原文、URL、reasoning、opaque 或凭据。

Run/attempt 归属、指标解释与真实调用预算只由[Probe 指南](probes.md)维护；诊断不触发 retry/backoff，不把 Retry-After 透传下游。嵌入方可在共享 Gateway 前调用 `with_probe_diagnostics(path)`，结束时调用 `flush_probe_diagnostics().await`。

## 嵌入与验证

`Gateway::new` 接收编译 topology、显式 entries、凭据绑定与 limits，不读取环境。`router()` 的嵌入方维护 listener 安全边界；`serve()` 自行检查 loopback 并连接 shutdown，丢弃 serve future 也取消 owned workers。跨协议 entry 仍受相同 IR/投影约束。

独立 owning-layer、Router/binary 与固定 SDK loopback 检查的职责和命令归[开发指南](development.md)。真实外部验证须按[Probe 指南](probes.md)另行授权；局部检查不证明一般 Agent、TLS/网络、缓存收益、负载或生产稳定性。运行结果不保存在本页。
