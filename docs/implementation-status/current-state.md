# 当前实现

本文只记录当前 checkout 已存在的产品行为、执行边界、源码 owner 和确定性证据入口。未实现、未验证和外部证据适用范围统一见[当前状态边界](current-boundaries.md)；模块地图见[当前架构](current-architecture.md)；带日期的真实 Provider、SDK 或 Agent 记录见[evidence](evidence/README.md)。

## 1. 网关入口与认证

- 未认证入口为 `GET /healthz`、`GET /openapi.yaml`、`GET /swagger-ui` 与 `GET /swagger-ui/`。
- Bearer 保护入口包括标准/扩展 Models、`POST /v1/chat/completions`、`POST /v1/responses`、`POST /v1/embeddings`、`POST /v1/images/generations`，以及 MCP 的 `POST /mcp` 与 legacy session `GET/DELETE /mcp`。
- 认证、请求 ID、body budget、敏感 header 标记和 tracing middleware 在业务 handler 前执行；认证失败返回 `401` 与 `WWW-Authenticate: Bearer`。
- MCP 同时支持 `2026-07-28` stateless discovery 和 legacy initialize/session/SSE/delete lifecycle，共享静态 `hello` tool；该 tool 不访问 registry、credential 或 Provider。
- Models DTO 只包含下游安全事实，不公开 Provider、Target、Route、upstream model、endpoint、credential、健康状态或价格。
- 能力拒绝使用 OpenAI-compatible 400 envelope 和标准顶层 `param`；内部 candidate、Route 和 capability reason 不序列化。

主要 owner：`src/ingress/router.rs`、`src/ingress/auth.rs`、`src/ingress/lifecycle.rs`、`src/ingress/mcp/`、`src/registry/public_model.rs`。

确定性入口：`tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs`、`tests/mcp_contract.rs`、`tests/mcp_dual_era.rs`。

## 2. 配置、凭证与静态注册

- `config/bootstrap.toml` 拥有 listener、limits、共享 HTTP client、默认 instructions、本地下游内容日志开关及可选 OTLP traces/metrics；省略日志字段或 signal table 分别表示关闭。
- 私有 `users.toml` 只提供下游用户/API key；私有 `upstream-credentials.toml` 只激活代码注册的 API-key pool 或 OAuth auth-file locator，不能新增 Provider、Target、Route、endpoint 或能力。
- 未知、重复、类型/Provider 不匹配或损坏 binding 在 listener 前失败；缺失或空 API-key pool 只禁用引用它的静态 Target。
- Rust catalog 显式注册闭合的 Provider family、canonical Model、Provider instance、credential pool、Upstream Target/API、Route 与 Public Model，并编译为 immutable `RuntimeRegistry`。
- Canonical Model、Provider Target 与 Public Model 的当前关系集中维护在[Model 与 Provider 映射](model-provider-mapping.md)；本页不复制单模型清单或 capability metadata。
- `models::{deepseek,qwen,z_ai}::tests` 中的 catalog contracts 固定近期模型 facts 及两个已移除 Qwen ID 的缺失状态；2026-08-24 已通过 `cargo fmt -- --check`、`cargo test --locked` 和 `cargo clippy --locked -- -D warnings`。
- Provider contract 是 capability ceiling；每个 Target/API 必须显式收窄。Public Model compiler 对全部固定 candidate 保守求交，请求能力不筛选、跳过或重排 candidate。
- Generation registration 显式选择 `NativeFirst` 或 `SourceFirst`；只在缺失下游协议 Native coverage 时为允许的单协议 source 补充 Bridge。Embeddings、Images 和专用音频 task 使用独立 operation contract。
- `openbridge-auth login chatgpt` 通过固定 device interaction 或 authorization-code + PKCE 取得完整 bundle，并事务写入 OpenBridge-owned auth file。常驻服务只在 guarded refresh 或首个预提交 401 recovery 中 reload/rotate。

主要 owner：`src/config/`、`src/models/`、`src/providers/`、`src/registry/`、`src/oauth2/`。

确定性入口：`tests/config_contract.rs`、`tests/example_config.rs`、`tests/upstream_credential_config.rs`、`tests/startup_contract.rs`、`tests/oauth2_login_cli.rs`。

## 3. Public Model 与请求预检

- 标准 Models list/retrieve 输出四字段对象；扩展 Models 输出 task、operation、modality、reasoning、state、streaming、typed multimodal contract 和 `supported_parameters` 等安全投影。
- 每个 operation interface 在启动期由全部固定 candidate 保守聚合；未知事实保持 unknown。
- analyzer 只冻结协议结构和请求事实，不查询 registry、不选择 Route。preflight 针对选定 Public Model 执行一次固定接口、limit 和 state 校验，通过后才按静态顺序生成 plan。
- 协议目录外字段返回 `unknown_parameter`；标准已知但固定接口不支持的字段返回 `unsupported_model_capability`。两者都在 Provider egress 前失败。
- reasoning、function tool、tool choice、structured output、stream usage、Responses `include`、prompt-cache、state/continuation 及图片、文件、音频能力均使用 typed profile，而不是请求时猜测或 capability routing。
- `prompt_cache_key` 由公共 interface 接受并按 candidate 的 concrete API 精确转发或删除；nullable prompt-cache no-op 在 planning 前移除，active retention 仍 fail closed。`parallel_tool_calls` 只有存在可执行 function tool 时才激活，inactive 值删除；active true 要求 toggleable contract，active false 可精确转发或由显式 serial-only candidate 安全删除，未知事实继续 fail closed。
- Responses 当前只接受省略或 `store:false`。continuation/state 只有全部 candidate 共享 issuing Target/API/credential affinity 时才可公开。

主要 owner：`src/pipeline/generation/`、`src/pipeline/embeddings/`、`src/registry/public_model/`。

确定性入口：`tests/forwarding_contract.rs`、`tests/ingress_contract.rs`、`tests/provider_boundary_contract.rs`。

## 4. Generation Native 与 Protocol Bridge

- 已注册且通过 preflight 的 Chat/Responses 请求可执行非流式 JSON 或 SSE Native 转发；Provider adapter 固定 upstream model、相对 path、安全 header 与 purpose-bound credential。
- response headers、首个 SSE event、event idle、可选 stream total 与非流式 total timeout 分开建模。首个有效下游事件前使用有界 single-event precommit；提交后不得 fallback 或拼接第二条响应。
- Chat ↔ Responses 只在显式 `Bridged` Route 上转换，支持 allowlist 内 text、function tool、parallel tool call、tool result、structured output、明文 reasoning 与可证明的 usage 映射。
- image、file、audio、hosted/custom tool、background/state 和 opaque continuation 没有可验证等价物时，Bridge 在 egress 前拒绝。
- SSE state machine 维护 item/call/index、fragmented arguments、terminal、EOF、body error 和 cancel；不会伪造 terminal 或把已提交 partial stream 改写为新响应。

主要 owner：`src/ingress/forwarding.rs`、`src/ingress/streaming.rs`、`src/pipeline/generation/`、`src/bridge/`、`src/provider/`、`src/transport/`。

确定性入口：`tests/forwarding_contract.rs`、`tests/sse_contract.rs`、`tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs`、`tests/protocol_bridge_replay.rs`、`tests/process_replay_contract.rs`。

## 5. Retry、fallback、cooldown 与取消

- 首个下游业务输出前按固定 candidate 顺序执行有界 retry/fallback；请求不能创建、筛选或重排 Route。
- 429 可在同一 credential pool 轮换有序 member；member/generation cooldown 与 target fault-domain cooldown 在单进程内共享。
- candidate retry 耗尽后只沿同一 Public Model 的注册 Route fallback；首个业务 body byte 提交后不得切换或拼接响应。
- 下游取消终止 send、backoff、response body 和后续 attempt；timeout、terminal、EOF-before-terminal、body error 与 cancel 各收口一次。
- Embeddings 复用 attempt/cancel 边界；Images 使用单 candidate、单 credential、单 physical attempt，不调用 recovery API。

主要 owner：`src/ingress/forwarding/`、`src/ingress/attempt.rs`、`src/ingress/health.rs`、`src/ingress/streaming.rs`。

确定性入口：`tests/forwarding_contract/resilience.rs`、`tests/process_replay_contract.rs`、`tests/embedding_forwarding_contract.rs`、`tests/images_forwarding_contract.rs`。

## 6. 扩展 operation

| Operation | 当前可执行合同 | 主要确定性证据 |
|---|---|---|
| Embeddings | `text-embedding-3-small`、`qwen3.7-text-embedding`、`nemotron-3-embed-1b` 各有独立 Public Model 和唯一 Native Route；成功 JSON 在 commit 前有界验证。 | `tests/embedding_forwarding_contract.rs` |
| Native 图片输入 | `mimo-v2.5`、DeepSeek Vision 及 image-capable Bailian Qwen/Kimi Native interface 接受各自有界的 HTTPS URL 或规范 Base64 data URL；Bailian Qwen 公开 250 张 BMP/JPEG/PNG/TIFF/WebP/HEIC 上游 envelope，Bailian Kimi 保持单张 JPEG/PNG；DeepSeek Vision 支持双 native 协议的 JPEG/PNG/GIF/WebP、显式 detail 与多图。 | `tests/forwarding_contract.rs` |
| Native 文件输入 | Chat 与 Responses 使用独立 typed file profile；当前 executable Target 均显式关闭 file，因此生产 Public Model 不公开文件输入。 | `tests/forwarding_contract/file_input.rs` |
| MiMo 音频 | `mimo-v2.5` Chat 支持有界 WAV 音频理解；ASR、TTS、VoiceDesign、VoiceClone 是独立 Chat-only task/Public Model。 | `tests/forwarding_contract.rs` |
| Images Generations | `qwen-image-3.0` 与 `qwen-image-3.0-pro` 通过 Bailian/DashScope Native endpoint 提供同步 URL JSON；OpenAI 标准字段为主合同，typed DashScope extensions 为显式扩展。 | `tests/images_forwarding_contract.rs` |

完整请求、能力和资源限制由[扩展能力需求](../functional-requirements/extended-capabilities.md)拥有；本页不复制其验收合同。

## 7. Provider 注册

当前 Model、Provider Target、候选顺序和 Public Model 关系见[Model 与 Provider 映射](model-provider-mapping.md)。运行时可见性还受 active credential pool 收窄；静态映射不表示实时可达、账号 entitlement 或真实 Provider 验收。字段级能力以 registry source 与运行中的扩展 Models API 为准；带日期的真实观察统一链接到[evidence](evidence/README.md)。

## 8. 观测与测试资产

- request lifecycle、Provider attempt、OTLP traces/metrics 和本地 bounded HTTP JSONL snapshot 分属独立 owner；内容 snapshot 只在认证后和显式开关下采集，并强制脱敏。
- 普通 observation 不记录 prompt、媒体正文、向量、credential、真实 endpoint、上游 body 或 Provider request ID。
- Rust tests 拥有 registry、routing、Provider wire、Bridge、retry/fallback/cooldown、取消和观测不变量；Python corpus/testkit 拥有 canonical corpus、fragmentation、standalone mock/client、报告与打包。
- canonical `testdata/` 是合同资产；`testdata/runtime/`、`generated/`、`reports/`、`dist/` 是可重建派生产物。

主要 owner：`src/observability/`、`testdata/`、`tools/corpus/`。

确定性入口：`tests/observability_contract.rs`、`tests/otlp_trace_contract.rs`、`tests/metrics_contract.rs`、`tools/corpus/tests/`。
