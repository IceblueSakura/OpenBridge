# OpenBridge 使用手册

OpenBridge 是一个面向本地或所有者控制环境的 headless、多 Provider、OpenAI-compatible 网关。它把代码中注册的
Provider、上游 Target、Route 和 Public Model 组合成一个固定的下游接口，并使用启动时加载的私有用户表认证本地客户端。

本 README 只说明当前 checkout 的安装、配置、运行和调用方式。实现状态、协议边界、测试证据与源码结构分别见
[文档总索引](docs/README.md) 和 [实施现状目录](docs/implementation-status/README.md)。

> 当前项目仍处于实验性原型阶段，没有已发布版本或外部兼容基线。默认只监听 loopback；不要直接把它当作公网多租户服务部署。

## 1. 你可以用 OpenBridge 做什么

当前服务提供：

- Chat Completions：`POST /v1/chat/completions`；
- Responses：`POST /v1/responses`；
- Embeddings：`POST /v1/embeddings`；
- MCP 本地测试服务：`POST /mcp`，当前提供 discovery、工具列表和 `hello` 调用；
- 标准和扩展 Models 查询；
- 可选的 OpenTelemetry traces 与 metrics OTLP/HTTP 导出；
- 管理员显式执行的上游 Models 发现和基础 API 探测。

### 使用优先级：无状态服务优先

OpenBridge 的核心实现重点是无状态服务，也是默认使用方式和当前验收基线。客户端应在每次请求中携带所需的完整历史，
优先省略 `store`、`previous_response_id` 和 `background`，或分别使用 `store: false`、`previous_response_id: null`
以及 `background: false`。

`previous_response_id`、`background` 和 `store: true` 属于次要目标，当前支持不完整，不能作为通用会话、后台任务或响应资源
服务使用。当前实现没有完整的 response state storage、retrieve/cancel、conversation lifecycle 或 continuation ledger；涉及
状态的请求还必须受所选 Public Model、唯一 issuing Target/API 和 Native-only 边界约束。正常接入、客户端示例和验收都应优先
采用无状态请求。

所有下游请求只能选择 Public Model。客户端不能提交上游 URL、Provider、Route、credential、认证 header 或 header
转换规则；这些信息由可信 Rust 注册表固定决定。请求先经过所选 Public Model 的能力预检，再按已注册 Route 顺序执行
Native 转发、受限 Chat ↔ Responses Bridge、有限 retry 或首个下游业务输出前的 fallback。

每个 generation Public Model 都显式选择类型化 Route 策略。`NativeFirst` 对每个下游协议先排列所有 Native，再排列 Bridge；
`SourceFirst` 先保持 source 优先级，再在同一 source 内优先 Native。`gpt-5.6-sol` 使用 `SourceFirst` 让 Chat 与 Responses 都优先
ChatGPT source；`deepseek-v4-flash` 也使用 `SourceFirst`，其 Chat source 固定为 DeepSeek、Bailian、OpenRouter。缺失协议仍可从相反
Native protocol 自动补充 Bridge；显式 Bridge surface 可以在已有其他 Native source 时保留。task-specific 音频等明确不跨协议的
surface 仍保持关闭。

## 2. 当前可调用模型

下面是代码中注册的 Public Model。实际运行时只有拥有可用 credential pool、且至少存在一条可执行 Route 的模型才会出现在
`/v1/models`；因此始终以运行中的 Models 接口为准。

| Public Model | 可用接口 | 典型 credential pool | 说明 |
|---|---|---|---|
| `gpt-5.6-sol` | Chat、Responses | `chatgpt-codex`、`openai-primary` | 两协议均优先 ChatGPT；OpenAI 为后备 source；公共能力按全部固定候选的交集公开 |
| `gpt-5.3-codex-spark` | Chat、Responses | `chatgpt-codex` | ChatGPT Responses Native；Chat 通过受限 Chat→Responses Bridge；下游支持 JSON/SSE |
| `gpt-5.5` | Chat、Responses | `chatgpt-codex` | ChatGPT Responses Native；Chat 通过受限 Chat→Responses Bridge；下游支持 JSON/SSE |
| `gpt-5.6-luna` | Chat、Responses | `chatgpt-codex` | ChatGPT Responses Native；Chat 通过受限 Chat→Responses Bridge；下游支持 JSON/SSE |
| `gpt-5.6-terra` | Chat、Responses | `chatgpt-codex` | ChatGPT Responses Native；Chat 通过受限 Chat→Responses Bridge；下游支持 JSON/SSE |
| `LongCat-2.0` | Chat、Responses | `longcat-primary` | Native-first + Bridge；公开 none/high 与明文 reasoning |
| `deepseek-v4-pro` | Chat、Responses | `deepseek-primary`、`bailian-primary` | DeepSeek/Bailian Chat Native；Responses 自动走 Chat Bridge；公开 none/high/max、明文 reasoning 与 `json_object` |
| `deepseek-v4-flash` | Chat、Responses | `deepseek-primary`、`bailian-primary`、`openrouter-primary` | `SourceFirst`；Chat 按 DeepSeek、Bailian、OpenRouter，Responses 按 DeepSeek、OpenRouter；公开 none/low/high/max 与 `json_object` |
| `minimax-m3` | Chat、Responses | `openrouter-primary`、`nvidia-primary` | OpenRouter Chat/Responses Native 优先、NVIDIA Chat Native 后备；两接口公开 none/high |
| `kimi-k3` | Chat、Responses | `kimi-primary` | Moonshot 中国区 endpoint Chat Native；Responses 自动通过 Chat Bridge，公开 none/low/high/max |
| `glm-5.2` | Chat、Responses | `bailian-primary` | 阿里云百炼北京 endpoint Chat Native；Responses 自动通过 Chat Bridge，公开 none/high/xhigh |
| `qwen3.7-plus` | Chat、Responses | `bailian-primary` | 百炼双协议 Native；两接口公开七档；Chat plain_text、Responses summary reasoning |
| `qwen3.7-max` | Chat、Responses | `bailian-primary` | 百炼双协议 Native；两接口公开七档；Chat plain_text、Responses summary reasoning |
| `qwen3.8-max` | Chat、Responses | `bailian-primary` | 百炼双协议 Native；两接口公开七档；Chat plain_text、Responses summary reasoning |
| `qwen3.6-27b` | Chat、Responses | `bailian-primary` | 百炼 Chat Native；Responses 通过 Chat Bridge；公开 none/high，不公开图片、视频或工具能力 |
| `mimo-v2.5-pro` | Chat、Responses | `mimo-primary` | 双协议 Native；两接口公开 none/low/medium/high；不公开图片输入 |
| `mimo-v2.5` | Chat、Responses | `mimo-primary` | 双协议 Native；两接口公开 none/low/medium/high；支持受限 URL/Base64 图片 |
| `mimo-v2.5-asr` | Chat | `mimo-primary` | MiMo 专用 ASR；单个 WAV `input_audio` + `asr_options`，不提供 Responses 或 `/audio/transcriptions` |
| `mimo-v2.5-tts` | Chat | `mimo-primary` | MiMo 预置音色 TTS；Chat `audio` 输出，非流式 WAV、流式 PCM16 |
| `mimo-v2.5-tts-voicedesign` | Chat | `mimo-primary` | MiMo 文本描述音色设计；Chat `audio` 输出，不接收 reference audio |
| `mimo-v2.5-tts-voiceclone` | Chat | `mimo-primary` | MiMo reference-voice cloning；Chat `audio.voice` conditioning + audio 输出 |
| `text-embedding-3-small` | Embeddings | `openai-primary` | 独立 Embeddings Native Route；不支持 streaming 或 Bridge |
| `qwen3.7-text-embedding` | Embeddings | `bailian-primary` | 百炼 Embeddings Native；支持固定维度集合；不支持 streaming 或 Bridge |

Reasoning level 是 Model 能力，同一模型的 Chat/Responses interface 公开同一集合。MiMo 官方当前把 `low`、`medium`、`high`
都解释为开启 reasoning，但 OpenBridge 仍在 Native Responses 中原样传递每个已声明值；Qwen3.7 与 Qwen3.8 同理保留官方七档。
MiniMax M3 与 Qwen3.6 27B 当前都只有 thinking 开关证据，因此统一公开 `none/high`，不外推未声明的中间强度档位。
只有 thinking 开关的 Chat API 将 `none` 编码为关闭、其余该模型已声明档位编码为开启，不因此缩减 Models 契约。

`text-embedding-3-small` 当前公开 `encoding_format`、`user` 和固定的 Embeddings 输入契约；显式 `dimensions` 不公开。
`qwen3.7-text-embedding` 当前公开 string/string-array 输入、float `encoding_format`、`dimensions` 及其固定允许值，默认维度为
1024，批量上限为 20，单输入 token 上限为 128000；不公开 `user`，也不支持 streaming 或 Bridge。
代码中已绑定 Provider Target 但未加入 Public Model/Route 的 canonical profile 仍不代表可调用模型。当前
`openai/gpt-5.5`、`openai/gpt-5.6-luna` 和 `openai/gpt-5.6-terra` 已分别绑定 OpenAI Target，但尚未加入独立 OpenAI source 的
Public Model/Route；其中 `gpt-5.5`、`gpt-5.6-luna` 和 `gpt-5.6-terra` 当前由 ChatGPT source 提供。

## 3. 前置条件与安全边界

开始前需要：

- Rust 2024 edition 工具链和 `cargo`；
- 当前 Provider 可用的上游 API key，或 ChatGPT 的 OpenBridge-owned OAuth2 文件；
- 一个用于下游客户端的本地 Bearer API key；
- 从仓库根目录执行命令。

凭证规则：

- `config/users.toml` 和 `config/upstream-credentials.toml` 是私有文件，仓库只提供 `.example.toml` 模板；
- 不要把真实 key、OAuth token、auth 文件或请求正文写入 README、fixture、日志或 Git；
- OpenBridge 不从上游 API key 环境变量或 `.env` 读取凭证，也不会导入本机 Codex auth cache；
- 用户、Provider、Model、Target 和 Route 不能由请求动态创建或选择；
- 服务和探测工具都使用代码注册的固定 endpoint，业务请求不能改写上游地址；
- 默认 listener 只能是 loopback 地址，bootstrap 不能直接暴露公网端口。

## 4. 安装与快速启动

### 4.1 构建二进制

在仓库根目录执行：

```bash
cargo build --locked
```

PowerShell 使用相同命令即可。`config/bootstrap.toml` 已随仓库提供，正常情况下不需要复制；它默认引用
`config/users.toml` 和 `config/upstream-credentials.toml`。

### 4.2 创建私有配置

复制两个模板：

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
```

PowerShell：

```powershell
Copy-Item config/users.example.toml config/users.toml
Copy-Item config/upstream-credentials.example.toml config/upstream-credentials.toml
```

然后根据模板中的注释编辑私有文件，不要把 placeholder 当作可用凭证：

- [config/users.example.toml](config/users.example.toml)：下游用户表结构和 API key 占位符；至少保留一个启用用户，用户 API key
  长度至少为 32 字节。
- [config/upstream-credentials.example.toml](config/upstream-credentials.example.toml)：全部已注册 credential pool、API-key
  与 OAuth2 写法，以及未启用 pool 的处理方式；只填写实际要启用的 pool。

具体字段、pool ID 和示例值以这两个 example 文件为准，私有文件不要提交到 Git。

多把 API key 可以按顺序放在同一个 `api_keys` 数组中。上游 `429` 等可重试情况可能触发同一 pool 内的
credential rotation；这不等于账号级负载均衡。

没有填写的 pool、没有 source 的 pool，或 `api_keys = []` 会使引用它的 Target 在本次启动中不可用，但不会从代码
注册表删除 Provider 或 Model。source 类型与注册表不匹配、重复 binding、空白或重复 key 会直接阻止启动。

`openai-primary` 是可选的 API-key pool。省略它或保留 `api_keys = []` 会按预期禁用 `openai-main`、三个新增的
OpenAI generation Target 以及 `openai-text-embedding-3-small`，但不会删除这些代码绑定；ChatGPT Public Model 使用独立的
`chatgpt-codex` OAuth2 pool，不受此设置影响。

`openrouter-primary` 激活 `deepseek-v4-flash` 的 OpenRouter 后备和 `minimax-m3` 的第一双协议 source；`nvidia-primary` 激活
`minimax-m3` 的 NVIDIA Chat 后备。`kimi-primary` 激活 `kimi-k3`；`bailian-primary` 激活两个 DeepSeek 的 Bailian Chat source、
`glm-5.2`、`qwen3.7-plus`、`qwen3.7-max`、`qwen3.8-max` 与 `qwen3.7-text-embedding`。填入相应 key 并重启后，启动编译器才会保留
引用该 pool 的 Target 与 Public Model；空数组仍保持这些入口不可用。

### 4.3 启动参数与环境变量

主服务不提供 `--config`、`--listen` 等命令行覆盖参数；监听地址、文件路径、资源限制和 OTLP 选择都由 Bootstrap 文件控制。
三个二进制的入口参数如下：

| 程序 | 参数或环境变量 | 说明 |
|---|---|---|
| `openbridge`、`openbridge-auth`、`openbridge-probe` | `OPENBRIDGE_CONFIG=<path>` | 选择启动时读取的 Bootstrap 文件；默认 `config/bootstrap.toml`。该变量只选择文件，不会新增 Provider、Model、Target 或 Route。 |
| `openbridge` | 无命令行参数 | 启动网关服务；使用 `Ctrl+C` 优雅停止。 |
| `openbridge` | `RUST_LOG=<filter>` | 设置本地日志过滤器；未设置时默认为 `info`，例如 `RUST_LOG=debug`。 |
| `openbridge-auth` | `login chatgpt` | 执行唯一支持的 ChatGPT device login；不接受 issuer、endpoint、auth-file 等覆盖参数。 |
| `openbridge-auth` | `--help`、`-h` | 显示固定命令用法。 |
| `openbridge-probe` | `--target <id>` | 必填，选择一个已注册且已启用的 Upstream Target。 |
| `openbridge-probe` | `--list-models`、`--chat`、`--responses`、`--embeddings`、`--all` | 选择 probe 类型；未指定时默认执行当前 target 的全部基础 probe，可组合使用。 |
| `openbridge-probe` | `--help`、`-h` | 显示 probe 用法。 |

Cargo 运行二进制时，传给二进制的参数要放在 `--` 后面，例如：

```bash
cargo run --locked --bin openbridge-auth -- login chatgpt
cargo run --locked --bin openbridge-probe -- --target openai-main --list-models
```

PowerShell 可在启动前设置环境变量：

```powershell
$env:OPENBRIDGE_CONFIG = "config/bootstrap.local.toml"
$env:RUST_LOG = "debug"
cargo run --locked --bin openbridge
```

### 4.4 启动服务

```bash
cargo run --locked --bin openbridge
```

默认 info 日志会先输出两张 `configuration only` 双列表格，分别列出配置态可用/不可用的 Provider family 和 Public Model。
Provider 项包含 enabled/total Target 计数，Model 项包含当前可执行的 Chat、Responses 或 Embeddings 接口；不可用项只给出脱敏原因。
表格不会显示 credential、pool、Target、Route 或 endpoint，也不会访问 Provider。它只说明当前启动配置是否形成执行候选，不代表网络、
配额、远端模型或基础 API 已经通过真实探测；上线前检查仍须显式运行 `openbridge-probe`。

默认地址是 `http://127.0.0.1:8080`。启动成功后，另开一个终端检查：

```bash
curl -i http://127.0.0.1:8080/healthz
```

`/healthz` 只检查本地进程和编译注册表，不会请求任何真实 Provider。成功响应包含 `status: "ok"` 和当前
`registry_version`。

按 `Ctrl+C` 优雅停止服务。修改 bootstrap、用户文件或上游 credential TOML 后需要重启；这些 TOML 不提供通用热重载。

### 4.5 最小调用

将 `replace-with-a-local-client-token` 替换为 `users.toml` 中启用用户的 `api_key`：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"hello"}]}'
```

Windows PowerShell 中如果 `curl` 被映射为 `Invoke-WebRequest`，请使用 `curl.exe`，或改用 PowerShell 的 HTTP
请求命令。

### 4.6 `mimo-v2.5` Native 图片理解

`mimo-v2.5` 的 Chat 与 Responses interface 都公开类型化 `multimodal_input.image`。下面两个请求分别走同协议 Native Route；
`mimo-v2.5-pro`、任何 Chat ↔ Responses Bridge、`file_id` 和显式 `detail` 不在该能力内。

Chat Completions：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"mimo-v2.5","messages":[{"role":"user","content":[{"type":"image_url","image_url":{"url":"https://example.com/image.png"}},{"type":"text","text":"Describe the image."}]}]}'
```

Responses：

```bash
curl http://127.0.0.1:8080/v1/responses \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"mimo-v2.5","input":[{"role":"user","content":[{"type":"input_image","image_url":"https://example.com/image.png"},{"type":"input_text","text":"Describe the image."}]}]}'
```

远程来源必须是有界的绝对 HTTPS URL；inline 来源使用规范的
`data:image/<format>;base64,<payload>`，当前允许 JPEG、PNG、GIF、WebP 和 BMP。OpenBridge 另施加每请求最多 64 个图片 part、
单 URL 最多 8192 UTF-8 字节及启动配置中的总请求体上限。默认 `max_request_body_bytes` 仅为 1 MiB，因此它会先于 MiMo 文档中的
50 MB 单图上游上限限制较大的 Base64 请求。

## 5. Bootstrap 配置

默认 bootstrap 文件是 `config/bootstrap.toml`。如需选择其他文件，使用[启动参数与环境变量](#43-启动参数与环境变量)中的
`OPENBRIDGE_CONFIG`；它只选择文件位置，不会新增 Provider 或修改注册表。Bootstrap 的完整可复制示例见
[config/bootstrap.example.toml](config/bootstrap.example.toml)，当前字段和默认值如下：

| 字段 | 默认值 | 作用 |
|---|---:|---|
| `schema_version` | `2` | bootstrap schema 版本 |
| `listen` | `127.0.0.1:8080` | loopback listener，只接受 loopback 地址 |
| `users_file` | `config/users.toml` | 下游用户文件 |
| `upstream_credentials_file` | `config/upstream-credentials.toml` | 上游 credential binding 文件 |
| `max_request_body_bytes` | `1048576` | 普通请求 body 上限，1 MiB |
| `max_json_response_body_bytes` | `16777216` | JSON 成功体上限，16 MiB |
| `max_replay_body_bytes` | `262144` | 可重放请求 body 上限，256 KiB |
| `max_sse_event_bytes` | `262144` | 单个 SSE event 上限，256 KiB |
| `upstream_connect_timeout_ms` | `5000` | 上游连接超时 |
| `upstream_pool_idle_timeout_ms` | `90000` | 上游连接池 idle 超时 |
| `upstream_pool_max_idle_per_host` | `16` | 每个 host 的最大 idle 连接数 |

所有 limit 和 timeout 必须为非零值，`max_replay_body_bytes` 不能超过 `max_request_body_bytes`。监听地址不能改成
`0.0.0.0` 或其他非 loopback 地址。

### 启用 OpenTelemetry OTLP/HTTP 导出

traces 与 metrics 默认都不导出，可以分别启用。确认 collector 是配置所有者明确选择的可信目标后，参考
[config/bootstrap.example.toml](config/bootstrap.example.toml) 中的 `[telemetry.traces]` 和 `[telemetry.metrics]` 段，
将对应配置复制到实际 bootstrap 文件。

该值必须是没有用户名、密码、path、query 和 fragment 的绝对 `http` base URL。OpenBridge 固定发送到
`/v1/traces` 和 `/v1/metrics`，不接受请求级 exporter 覆盖、自定义 exporter header 或环境注入的 header。metrics 使用
OpenTelemetry SDK 的累计 Counter/Histogram 聚合与固定 60 秒采集间隔，并在进程关闭时执行有界 flush。collector 不可用时
只丢弃 telemetry，不改变业务响应或上游选择。

当前没有 OTLP logs、内置 Prometheus exporter、持久化 metrics 或分布式聚合。

## 6. ChatGPT OAuth2（可选）

ChatGPT Public Model 使用独立的 `chatgpt-codex` OAuth2 credential pool。首次使用前，在
`config/upstream-credentials.toml` 中设置 `chatgpt-codex` 的 `auth_json_file`。字段形状和路径占位符见
[config/upstream-credentials.example.toml](config/upstream-credentials.example.toml)。

相对路径按该 TOML 文件所在目录解析。然后执行唯一支持的管理员命令：

```bash
cargo run --locked --bin openbridge-auth -- login chatgpt
```

命令会显示固定 verification URI 和一次性 device code，完成 device authorization、PKCE exchange 后，把完整 bundle
事务性写入配置指定的 OpenBridge-owned auth 文件。不要分享 device code，不要复制本机 Codex 的 auth 文件。

该命令不接受 issuer、client、endpoint、header、auth-file 或其他 cache override。常驻服务只负责到期驱动的 refresh
和一次有界的 `401` recovery；不提供运行时切换账户，也不会自动开始交互式登录。

登录后重启服务，并使用下面五个包含 ChatGPT source 的 Public Model 之一：

```text
gpt-5.3-codex-spark
gpt-5.5
gpt-5.6-luna
gpt-5.6-terra
gpt-5.6-sol
```

ChatGPT 上游固定使用 Responses SSE。下游可以使用 `/v1/responses` Native，或使用 `/v1/chat/completions` 进入受限
Chat→Responses Bridge；`stream: true` 直接返回经校验的 SSE，省略 `stream` 或使用 `stream: false` 时，OpenBridge 会强制上游
`stream: true`，在配置的 response budget 内完整校验 Responses lifecycle，并只在合法 terminal 后返回一个 JSON 对象。该转换开关
属于可信 Upstream API 注册，不是客户端字段。ChatGPT 登录不是本机 Codex credential、identity 或 executable probe。

## 7. 下游 API 使用

### 7.1 认证与公共资源

除下表中的公共资源外，所有下游 API 都要求：

```http
Authorization: Bearer <users.toml 中启用用户的 api_key>
```

| 方法 | 路径 | 认证 | 用途 |
|---|---|---|---|
| `GET` | `/healthz` | 否 | 本地健康与 registry version |
| `GET` | `/openapi.yaml` | 否 | 当前 OpenAPI 规范 |
| `GET` | `/swagger-ui`、`/swagger-ui/` | 否 | 本地 Swagger UI 测试页 |
| `GET` | `/v1/models`、`/v1/models/{model}` | 是 | 标准四字段 Models 对象 |
| `GET` | `/openbridge/v1/models`、`/openbridge/v1/models/{model}` | 是 | 扩展能力和参数契约 |
| `POST` | `/v1/chat/completions` | 是 | Chat Completions JSON/SSE |
| `POST` | `/v1/responses` | 是 | Responses JSON/SSE |
| `POST` | `/v1/embeddings` | 是 | Embeddings JSON |
| `POST` | `/mcp` | 是 | MCP `2026-07-28` discovery、`hello` 列表与调用 |

标准 Models 接口只返回客户端可用的 Public Model 身份，不返回 Provider、Target、Route、上游 model、endpoint、
credential、health 或 pricing。需要确定可用参数时，先读取扩展 Models：

扩展 generation interface 的 `reasoning.levels` 表示实际执行档位，`accepted_levels` 表示客户端可提交档位，`input_policy` 表示固定
转换规则。当前通用文本 generation Public Model 使用 `clamp_positive_floor`：正向 effort 向下落到不高于请求值的最高可执行档，低于
最小档时夹到最小档；`none` 仅在实际 `levels` 包含它时原样接受，永不转换为开启 reasoning。音频专用与 Embeddings Public Model
保持 `strict`。

```bash
curl http://127.0.0.1:8080/v1/models \
  -H 'Authorization: Bearer replace-with-a-local-client-token'

curl http://127.0.0.1:8080/openbridge/v1/models \
  -H 'Authorization: Bearer replace-with-a-local-client-token'

curl http://127.0.0.1:8080/openbridge/v1/models/text-embedding-3-small \
  -H 'Authorization: Bearer replace-with-a-local-client-token'

curl http://127.0.0.1:8080/openbridge/v1/models/qwen3.7-text-embedding \
  -H 'Authorization: Bearer replace-with-a-local-client-token'
```

### 7.2 Chat Completions

非流式请求：

```bash
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"Explain fallback in one sentence."}]}'
```

流式请求：

```bash
curl -N http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","messages":[{"role":"user","content":"Say hello."}],"stream":true}'
```

使用独立 ChatGPT Public Model 时，Chat 请求会进入受限 Chat→Responses Bridge：

```bash
curl -N http://127.0.0.1:8080/v1/chat/completions \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-luna","messages":[{"role":"user","content":"Say hello."}],"stream":true}'
```

`Content-Type` 必须是 `application/json`。生成接口中的工具调用只在协议 wire 层转发，OpenBridge 不执行这些 function tool；
独立 `/mcp` 只执行无外部 side effect 的本地 `hello` 测试工具。Bridge 只转换当前明确声明为可表达的共同语义；不可表达的字段会在访问上游前拒绝。

### 7.3 Responses

非流式请求：

```bash
curl http://127.0.0.1:8080/v1/responses \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","input":"Explain fallback in one sentence."}'
```

流式请求：

```bash
curl -N http://127.0.0.1:8080/v1/responses \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"gpt-5.6-sol","input":"Say hello.","stream":true}'
```

无状态请求是推荐路径：每次携带完整历史，并省略 `store`、`previous_response_id` 与 `background`。这些状态相关字段是否
可用取决于所选 Public Model 的固定 interface；不要根据 OpenAPI 的通用 schema 推断每个模型都支持它们。当前
`store: true`、非空 `previous_response_id` 和 `background: true` 不是通用可用能力，状态支持也不是当前默认验收范围。
当前 ChatGPT source 的 Responses 路径固定为 Native，Chat 路径为受限 Bridge；两种路径的下游都支持 JSON/SSE。ChatGPT 上游仍固定
`stream: true` 和 `store: false`；非流式 JSON 由 OpenBridge 在完整、合法、bounded 的 Responses SSE terminal 后生成。

### 7.4 Embeddings

Embeddings 是独立的 JSON-only 链路，不支持 streaming、Bridge 或向量转换。先读取扩展 Models，再发送其公开参数：

```bash
curl http://127.0.0.1:8080/v1/embeddings \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -d '{"model":"text-embedding-3-small","input":["alpha","beta"],"encoding_format":"float"}'
```

当前可接受的 input 形状包括 string、string array、token array 和 token-array array；`encoding_format` 默认是
`float`，默认维度为 1536。显式 `dimensions` 当前不公开，非法或超限的输入会在上游调用前拒绝。请求成功体会在下游
提交前执行有界 JSON 校验。

### 7.5 MCP 本地测试服务

`/mcp` 是为后续本地工具扩展建立的 MCP Streamable HTTP endpoint。当前只支持正式协议版本 `2026-07-28`、
`server/discover`、`tools/list` 和本地 `hello` 的 `tools/call`；没有 Provider Bridge、资源、prompt、session 或独立 SSE stream。

最小 discovery 请求：

```bash
curl http://127.0.0.1:8080/mcp \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2026-07-28' \
  -H 'Mcp-Method: server/discover' \
  -d '{"jsonrpc":"2.0","id":"discover-1","method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"local-client","version":"1.0.0"},"io.modelcontextprotocol/clientCapabilities":{}}}}'
```

调用 `hello(name: string)`：

```bash
curl http://127.0.0.1:8080/mcp \
  -H 'Authorization: Bearer replace-with-a-local-client-token' \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -H 'MCP-Protocol-Version: 2026-07-28' \
  -H 'Mcp-Method: tools/call' \
  -H 'Mcp-Name: hello' \
  -d '{"jsonrpc":"2.0","id":"hello-1","method":"tools/call","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientInfo":{"name":"local-client","version":"1.0.0"},"io.modelcontextprotocol/clientCapabilities":{}},"name":"hello","arguments":{"name":"Ada"}}}'
```

成功结果包含 `{"type":"text","text":"Hi, Ada!"}`。`hello` 只接受一个必需的字符串属性 `name`，不会读取配置、文件、
registry 或网络，也不会访问 Provider。

当前 endpoint 只接受不带 `Origin` 的本地客户端；任何 `Origin` 都返回 `403`。它不兼容 `2025-11-25` 及更早版本的
`initialize`/`initialized` 或 session lifecycle。新增其他工具仍需另立需求、焦点和工具安全边界。

### 7.6 响应、错误与重试边界

认证失败通常返回 `401 invalid_api_key`；未知 Public Model 返回 `404`；能力或参数不支持通常返回 `400`；
非 JSON 请求返回 `415`；上游不可用或超时通常归一为 `502` 或 `504`。响应会带 `X-Request-Id`，排障时请保留它。

Transient upstream failure 只允许在首个下游业务输出提交前执行有限 retry/fallback。首个下游业务 body byte 写出后，
不会切换上游、重试或拼接另一条响应；下游断开会取消当前上游请求、退避和后续 attempt。

## 8. OpenTelemetry 与 Swagger

### 8.1 OTLP metrics

启用 `[telemetry.metrics]` 后，OpenBridge 向 collector 导出下游请求、Provider attempt、retry/rotation/fallback/cooldown、
latency、TTFT、token usage、cache usage 和 output speed。标准生成式 AI 指标使用 `gen_ai.client.operation.duration` 与
`gen_ai.client.token.usage`；OpenBridge 特有生命周期使用 `openbridge.*` 命名空间。

指标属性只保留受信的 operation、Provider、Route、Target、模型、模式、streaming 和 outcome，不包含请求正文、响应正文、
Authorization、credential、用户、request ID 或 endpoint URL。历史、查询、dashboard、告警、rate/ratio 和跨进程聚合由外部
OpenTelemetry backend 负责；OpenBridge 不再提供自定义 metrics HTTP endpoint。完整口径见
[运行时指标与遥测](docs/implementation-status/telemetry-metrics.md)。

### 8.2 Swagger UI 和 OpenAPI

服务启动后可在浏览器打开：

- [Swagger UI](http://127.0.0.1:8080/swagger-ui/)；
- [OpenAPI YAML](http://127.0.0.1:8080/openapi.yaml)。

Swagger UI 是本地接口测试页。点击 `Authorize`，填入下游 Bearer API key 后可以测试受保护的 Models、
Chat、Responses 和 Embeddings。页面依赖固定版本的 jsDelivr 静态资源；规范本身由本地服务提供。

## 9. 上游 Models 与基础 API 探测

`openbridge-probe` 不启动下游网关，不修改代码注册表，只对一个已注册且已启用的 Upstream Target 发起显式探测，并输出
脱敏 JSON 报告。它读取当前 bootstrap 和上游 credential 配置，因此仍需要对应真实 credential。

常用 target ID（示例）：

```text
openai-main
openai-gpt-5-5
openai-gpt-5-6-luna
openai-gpt-5-6-terra
openai-text-embedding-3-small
bailian-qwen3-7-text-embedding
longcat-2
openrouter-deepseek-v4-flash
deepseek-v4-pro
deepseek-v4-flash
mimo-v2-5-pro
mimo-v2-5
chatgpt-gpt-5-3-codex-spark
chatgpt-gpt-5-6-luna
chatgpt-gpt-5-6-terra
chatgpt-gpt-5-6-sol
```

探测某个上游 Models 端点：

```bash
cargo run --locked --bin openbridge-probe -- --target openai-main --list-models
cargo run --locked --bin openbridge-probe -- --target longcat-2 --list-models
cargo run --locked --bin openbridge-probe -- --target chatgpt-gpt-5-6-sol --list-models
```

还可以选择 `--chat`、`--responses`、`--embeddings` 或 `--all`。例如：

```bash
cargo run --locked --bin openbridge-probe -- --target openai-main --chat --responses
cargo run --locked --bin openbridge-probe -- --target openai-text-embedding-3-small --embeddings
cargo run --locked --bin openbridge-probe -- --target bailian-qwen3-7-text-embedding --embeddings
cargo run --locked --bin openbridge-probe -- --target chatgpt-gpt-5-6-sol --responses
```

如果没有选择器，默认就是该 target 的全部基础 probe；`--all` 只表示当前一个 target 的 Models、Chat、Responses 与
Embeddings probe，不会遍历所有 target。未在该 target 注册的 operation 会报告 `unsupported` 且不发起对应请求。ChatGPT
Responses probe 使用其固定 streaming-only profile，并以 Provider adapter 识别的正常 SSE 终态作为成功条件。

探测成功只说明当前账号、网络、上游状态和固定请求在当时可用；认证失败、限流、网络错误或无效响应可能保守地记录为
`unknown`，不能据此推断生产配额或长期稳定性。基础 probe 不发送 tool 定义或 tool result，也不评测 function calling、
reasoning、结构化输出、多模态、模型语义质量、SDK/Agent 兼容性、retry/fallback、负载或长稳能力。

## 10. 常见问题

| 现象 | 检查方式 |
|---|---|
| 服务在监听前退出 | 检查 bootstrap schema、文件路径、loopback listener、非零 limit，以及 `max_replay_body_bytes <= max_request_body_bytes` |
| 请求返回 `401` | 检查 `Authorization: Bearer ...` 是否使用 `users.toml` 中启用用户的完整 key；用户 key 至少 32 字节 |
| 模型不在 `/v1/models` | 检查对应 credential pool 是否存在有效 source；缺失/空 pool 会禁用其 Target；ChatGPT 还要先完成显式 login |
| ChatGPT 返回需要重新认证 | 确认 `auth_json_file` 是 OpenBridge-owned 文件并重新运行 `openbridge-auth login chatgpt`；不要导入 Codex cache |
| `openbridge-probe` 报 target disabled | 检查该 target 引用的 pool 是否有非空 API key 或有效 OAuth2 auth-file，并确认 target ID 拼写 |
| 请求返回 `415` | 为 Chat、Responses、Embeddings 都设置 `Content-Type: application/json` |
| 请求参数被拒绝 | 先读取 `/openbridge/v1/models`；能力是 Public Model 固定契约，不是所有模型共享的并集 |
| `/healthz` 正常但业务失败 | 健康检查不访问 Provider；继续检查上游 credential、网络、Provider 状态和返回的 `X-Request-Id` |
| collector 没有 metrics | 确认已配置 `[telemetry.metrics]`、collector 接受 OTLP/HTTP protobuf `/v1/metrics`；默认采集间隔是 60 秒，正常关闭会 flush |
| 端口被占用 | 在 bootstrap 中把 `listen` 改为其他 loopback 地址/端口后重启，不能改为公网监听 |

## 11. 维护者验证

只修改 Rust 源码时，默认检查为：

```bash
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

修改 `testdata/` 或 `tools/corpus/` 时，再运行：

```bash
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

确定性 Rust test、fixture replay 和 loopback/SDK 检查不能替代真实 Provider、外部网络、目标 Agent、负载或长期运行
验收。执行这些扩展验收时，应记录实际环境、依赖版本和凭证边界。文档维护通常只需要内容/链接检查与
`git diff --check`，不要求完整运行时测试。

## 12. 进一步阅读

- [文档总索引](docs/README.md)：功能需求、实施现状、实施计划和参考文档的分类入口；
- [产品范围](docs/functional-requirements/product-scope.md)：单配置所有者部署、支持边界和非目标；
- [网关 API 与客户端兼容](docs/functional-requirements/gateway-api-compatibility.md)：下游 endpoint、JSON/SSE、tool 和 state 边界；
- [配置与凭证边界](docs/functional-requirements/configuration-and-credentials.md)：bootstrap、代码注册表和 secret trust boundary；
- [当前实现总览](docs/implementation-status/current-implementation.md)：已完成行为、横向能力和证据层级；
- [Provider 与模型注册表](docs/implementation-status/features/provider-registry-and-model-catalog.md)：Public Model、Target 和 active pool 行为；
- [ChatGPT OAuth2](docs/implementation-status/features/chatgpt-oauth-startup.md)：登录、refresh、Responses 数据面和固定 Models probe；
- [Models 与基础 API 探测](docs/implementation-status/capability-probing.md)：probe 输入、输出与不证明的范围；
- [config/bootstrap.example.toml](config/bootstrap.example.toml)、[config/users.example.toml](config/users.example.toml)、
  [config/upstream-credentials.example.toml](config/upstream-credentials.example.toml)：无真实凭证的配置模板；
- [docs/openapi.yaml](docs/openapi.yaml)：当前服务实际提供的机器可读接口规范。

## 开源协议

原创源代码与仓库文档采用 [MIT License](LICENSE)。参考项目只用于协议、行为和实现边界调研；后续引入外部代码、
测试或资源时，必须同时保留其许可证、版权声明和适用通知。
