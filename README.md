# OpenBridge 使用手册

OpenBridge 是一个面向本地或所有者控制环境的 headless、多 Provider、OpenAI-compatible 网关。它把代码中注册的
Provider、Upstream Target、Route 和 Public Model 编译为固定下游接口，并使用私有用户表认证本地客户端。

本 README 只负责安装、配置、启动、最小调用和常见排障。产品合同、当前实现、验证证据和外部协议资料分别见
[文档总索引](docs/README.md)与[实施现状目录](docs/implementation-status/README.md)。

> OpenBridge 仍是未发布的实验性原型。默认且只允许监听 loopback；不要直接把它作为公网多租户服务部署。

## 1. 当前入口

| 接口 | 认证 | 用途 |
|---|---|---|
| `GET /healthz` | 否 | 本地进程与注册表存活检查，不访问 Provider |
| `GET /openapi.yaml`、`GET /swagger-ui[/]` | 否 | 当前机器可读契约与本地测试页 |
| `GET /v1/models[/{model}]` | Bearer | 标准 Public Model list/retrieve |
| `GET /openbridge/v1/models[/{model}]` | Bearer | 带固定接口能力的扩展 Public Model 视图 |
| `POST /v1/chat/completions` | Bearer | Chat Completions JSON/SSE |
| `POST /v1/responses` | Bearer | Responses JSON/SSE |
| `POST /v1/embeddings` | Bearer | Embeddings JSON |
| `POST /mcp`；legacy `GET/DELETE /mcp` | Bearer | MCP dual-era discovery、legacy session/SSE lifecycle 与无副作用 `hello` 工具 |

下游客户端只能选择 Public Model，不能提交上游 URL、Provider、Route、credential、认证 header 或转换规则。实际可调用模型取决于
当前进程中配置态激活的 credential pool 和静态可执行 Route。运行中的 `/v1/models` 是“当前配置会公开哪些模型”的依据，
但它不探测 credential 是否有效、Provider 是否可达、配额或账号状态；真实可调用性仍需显式 probe 或实际请求验证。

Responses 的默认使用方式是由客户端携带完整历史的无状态请求。正常接入应省略 `store`、`previous_response_id` 和
`background`，或分别使用 `false`、`null` 和 `false`；当前 Public Model 未公开的状态能力会在 Provider egress 前拒绝。

## 2. 前置条件与私有配置

需要 Rust 2024 edition 工具链和 `cargo`，并从仓库根目录执行命令。

仓库提供可以提交的示例，但不提供可用 credential：

- [config/users.example.toml](config/users.example.toml)：下游用户与 Bearer API key 形状；
- [config/upstream-credentials.example.toml](config/upstream-credentials.example.toml)：已注册 credential pool、API key 与 OAuth2 文件绑定；
- [config/bootstrap.example.toml](config/bootstrap.example.toml)：完整 Bootstrap 字段、作用和安全边界。

创建私有文件：

```powershell
Copy-Item config/users.example.toml config/users.toml
Copy-Item config/upstream-credentials.example.toml config/upstream-credentials.toml
```

或在 Bash 中：

```bash
cp config/users.example.toml config/users.toml
cp config/upstream-credentials.example.toml config/upstream-credentials.toml
```

复制 upstream 示例后必须删除所有未使用的 `[[credential_pools]]`，或把对应 API-key pool 改成 `api_keys = []`；不得保留任何
`replace-with-*` placeholder。ChatGPT binding 只有在按第 6 节完成显式登录、生成有效 auth 文件后才能启用；暂不使用时应删除该
pool，而不是保留不存在的 `auth_json_file`。

然后只填写实际需要启用的 pool。`config/users.toml`、`config/upstream-credentials.toml` 和 OAuth auth 文件都是私有数据，
不得提交、打印或复制到日志、fixture、文档和问题报告。OpenBridge 不从 `.env`、上游 API-key 环境变量或本机 Codex auth cache
导入 credential。

未配置、没有 source 或 `api_keys = []` 的 pool 会禁用引用它的 Target；它不会删除代码中的 Provider、Model 或注册事实。
修改 Bootstrap、用户或 credential binding 后需要重启；修改编译期 Provider、Model 或 Route catalog 后需要重新构建并重启。

## 3. 构建与启动

构建：

```powershell
cargo build --locked
```

启动默认配置：

```powershell
cargo run --locked --bin openbridge
```

默认读取 `config/bootstrap.toml`，监听 `http://127.0.0.1:8080`。如需选择另一份 Bootstrap，只能通过
`OPENBRIDGE_CONFIG` 指定文件位置：

```powershell
$env:OPENBRIDGE_CONFIG = "config/bootstrap.local.toml"
$env:RUST_LOG = "info"
cargo run --locked --bin openbridge
```

服务不提供 `--listen`、`--provider`、`--endpoint` 或 credential 覆盖参数。启动成功后检查：

```powershell
curl.exe -i http://127.0.0.1:8080/healthz
```

`/healthz` 只证明本地进程和编译注册表可用，不证明真实 Provider、账号、配额或网络可用。

## 4. 最小调用

先查询当前配置态公开且具有静态执行候选的模型：

```powershell
curl.exe http://127.0.0.1:8080/v1/models `
  -H "Authorization: Bearer replace-with-a-local-client-token"
```

再把 `<public-model>` 替换为返回列表中的模型：

```powershell
curl.exe http://127.0.0.1:8080/v1/chat/completions `
  -H "Authorization: Bearer replace-with-a-local-client-token" `
  -H "Content-Type: application/json" `
  -d '{"model":"<public-model>","messages":[{"role":"user","content":"hello"}]}'
```

Bash 使用相同 URL、header 和 JSON 即可。PowerShell 中请显式使用 `curl.exe`，避免 Windows PowerShell 的 `curl` alias 改变参数语义。

请求是否支持 streaming、reasoning、tools、structured output、图片、音频或 Embeddings 特定字段，以
`/openbridge/v1/models/{model}` 返回的固定接口契约为准。该契约是所有固定可执行候选的保守交集，不会按请求跳过较弱 Route。

## 5. Bootstrap、日志与遥测

Bootstrap 拥有 listener、私有文件路径、请求/响应/SSE 上限、共享 HTTP client、默认 generation instructions、本地下游内容日志和
OTLP/HTTP exporter 配置。完整字段与注释以 [config/bootstrap.example.toml](config/bootstrap.example.toml) 为准。

### 本地下游内容日志

`[logging]` 包含 JSONL 目录和四个彼此独立的布尔字段：

```toml
[logging]
http_jsonl_directory = "/var/lib/openbridge/http-logs"
request_headers = true
request_body = true
response_headers = true
response_body = true
```

随附的 `config/bootstrap.toml` 和 `config/bootstrap.example.toml` 是受控开发 profile，显式把四项全部设为 `true`；自定义配置省略
整个表或任一布尔字段时，对应值解析为 `false`。启用任一开关时目录必须是绝对路径，OpenBridge 会在监听前创建并验证按 UTC 日期滚动的
`http-YYYY-MM-DD.jsonl`；普通运行日志仍写 stdout/journald，历史内容文件不自动删除。这些开关只观察通过 Bearer 认证后的最终下游
客户端边界，不是原始 Provider wire dump。

认证、Cookie、token、key、secret、password、session、credential 和 signature header 值始终脱敏。请求和响应正文捕获有界，
每个方向最多产生一个终态 snapshot，SSE 不按 chunk 记录。正文仍可能包含敏感业务内容，生产所有者必须在接入敏感流量前关闭或收窄
这些开发开关。

### OpenTelemetry

schema 省略对应 `[telemetry.*]` table 时，traces 或 metrics exporter 分别禁用；仓库随附的两个开发 Bootstrap profile 则显式
启用二者并指向 `http://127.0.0.1:4318`。collector base URL 必须是无用户信息、path、query 或 fragment 的绝对 `http` URL。
OpenBridge 固定发送到 `/v1/traces` 和 `/v1/metrics`，不提供请求级 exporter 覆盖、内置 Prometheus、metrics 查询 API、持久化或
分布式聚合。collector 故障不会改变业务响应或 Route 选择。

指标口径与敏感属性边界见[当前实现](docs/implementation-status/current-state.md)和
[当前状态边界](docs/implementation-status/current-boundaries.md#5-观测配置与生产边界)。

## 6. ChatGPT OAuth2（可选）

在 `config/upstream-credentials.toml` 的 `chatgpt-codex` binding 中设置 OpenBridge-owned `auth_json_file`，然后执行：

```powershell
cargo run --locked --bin openbridge-auth -- login chatgpt
```

命令会显示固定 verification URI 与一次性 user code（验证码）；私有 device code 不会显示。完成 private device interaction 和
PKCE exchange 后，命令事务性写入配置指定的 auth 文件。不要分享验证码，也不要导入或复制本机 Codex auth cache。

常驻服务可在固定账户绑定内执行到期驱动 refresh、guarded reload 和一次有界的预提交 `401` recovery，但不提供运行时切换账户或
自动交互登录。登录后重启服务，再以 `/v1/models` 确认当前配置会公开哪些 ChatGPT-backed Public Model；真实可调用性仍由 probe
或实际请求确认。

详细边界见[当前实现](docs/implementation-status/current-state.md)、
[配置与凭证合同](docs/functional-requirements/configuration/credentials.md)和
[Provider 接入进度](docs/implementation-status/providers/chatgpt.md)。

## 7. 显式 Provider 探测

`openbridge-probe` 不启动下游网关，也不修改注册表。它按管理员选择的 Provider 从已经注册且已启用的 Generation Target 取得 trusted
origin、Provider path/body hook、timeout 和 credential binding；只有多个 trusted deployment 需要 `--target` 显式消歧。随后工具执行
带认证的 Models 或固定合成请求并输出脱敏 JSON：

```powershell
cargo run --locked --bin openbridge-probe -- models --provider openai
cargo run --locked --bin openbridge-probe -- generation --provider bailian --model candidate-model-id --protocol chat --delivery non-streaming --case tool-parallel-true
```

`models` 输出 Provider 固定 Models endpoint 的完整计数、最多 1024 项有界 ID 样本，以及可选 `--model` 的可见性。
`generation` 需要 `--model`，一次只执行一个由 `--protocol`（chat/responses）、`--delivery`（non-streaming/streaming）和
`--case` 选择的请求；默认是 Chat、non-streaming 与 text。case 为
text/reasoning-none/reasoning-minimal/reasoning-low/reasoning-medium/reasoning-high/reasoning-xhigh/reasoning-max/json-object/
json-schema/json-schema-strict/image-input-inline-png/tool-auto/tool-none/tool-required/tool-named/tool-strict/tool-parallel-false/
tool-parallel-true。
CLI 和 library 不接受 `all`、列表或内置笛卡尔矩阵；外部测试脚本通过多次独立调用编排。
Structured case 携带固定冲突 prompt 与固定 `{"probe":"ok"}` schema；tool case 携带两个以内固定 function tools、固定 prompt 与
固定 arguments schema，只观察单次首轮响应中的 tool choice、strict 和 parallel 差分，不执行工具、不发送 tool result，也不发起
continuation。两类 case 都按完整 terminal 与瞬时输出给出 `supported`、`not_honored` 或 `inconclusive`，不保留生成文本、tool
arguments、call ID 或 item ID。`auto` 未调用工具和 `parallel=true` 只返回一个调用都记为 `inconclusive`，不误报不支持。

管理员可以为非 tool case 用 `--prompt <text>`（≤ 4 KiB）替换该 case 的固定用户 prompt，并可以为 `json-schema` /
`json-schema-strict` case 用 `--schema <json>`（≤ 8 KiB 的 JSON object）与 `--schema-name <name>` 替换响应格式对象与名称；
`--prompt` 对 tool case 拒绝，`--schema`/`--schema-name` 对其他 case 拒绝。带自定义 `--schema` 的 case 因无固定 oracle 而
恒为 `inconclusive` verdict，schema 接受性由 `accepted`/`rejected` outcome 体现；报告为每个生效覆盖记录
`custom_prompt_fingerprint`、`custom_schema_fingerprint`（各自内容的 SHA-256 前 16 位十六进制）与 `custom_schema_name`，
evidence 归属由外部脚本记录指纹与原文的对应，报告本体从不包含覆盖文本。无覆盖时全部 19 个 case 的 wire、oracle 与 verdict 保持
canonical 不变。
`image-input-inline-png` 使用内置、已视觉复核的固定 PNG data URL；Chat 发送 `image_url`，Responses 发送 `input_image`。只有完整响应
精确返回图片中的固定 token 才记为 `supported`，请求成功但识别不匹配记为 `inconclusive`，报告不保留图片 data URL、prompt 或输出正文。
`--target` 仅在多个 trusted deployment 之间显式消歧；Provider 解析只接受已注册且启用的 Generation Target。所有 bounded
Generation case 使用固定 4096-token accuracy-oriented upstream output limit；探测 Target 自身已注册 upstream model 时按其 output ceiling
下调，显式 candidate model 不继承另一模型的 ceiling。只有 backend 明确
拒绝该字段时才使用 `--allow-unbounded-streaming-output` 放开 streaming limit，这可能增加 reasoning 时间和计费。

`--model` 允许在正式注册前把同一个 candidate model ID 用于 `models` 可见性与 `generation` case；所选 Provider 解析只接受
Generation task Target，Embeddings/Images/Audio Target 不能借 Provider-wide path 发送 Generation。该参数不能覆盖 endpoint、
relative path、credential、认证 header 或任意 JSON 结构；`--prompt` 与 `--schema`/`--schema-name` 只替换上述固定合成请求的
用户 prompt 文本与响应格式对象，不改变 operation、工具定义或图片负载。
每个 case 独立报告 `accepted`、`rejected`、`unsupported` 或 `inconclusive`、HTTP status、耗时、标准 token usage、失败阶段及有界协议元数据；报告不包含
credential、认证 header、完整请求正文、生成正文或完整 upstream response body。
`unsupported` 只表示本地 trusted Target/profile 不允许 operation 或 delivery；真实 upstream 的所有非 2xx（包括 candidate-model 404）
都只是该请求的 `rejected`，不会提升为 endpoint 静态结论。

一次 `accepted` 或 capability oracle 的 `supported` 只证明该固定首轮请求当时取得相应 JSON/SSE 结果；它不证明 reasoning 参数实际生效、
完整工具调用流程、工具执行/续轮、能力稳定，或 inline PNG 之外的 remote/detail/其他多模态能力，也不证明模型质量、SDK/Agent 兼容、
retry/fallback、负载或长期稳定性。完整说明见
[当前状态边界](docs/implementation-status/current-boundaries.md)。

## 8. OpenAPI 与 Swagger UI

服务启动后可以访问：

- [Swagger UI](http://127.0.0.1:8080/swagger-ui/)；
- [OpenAPI YAML](http://127.0.0.1:8080/openapi.yaml)。

Swagger UI 是本地测试页，页面脚本来自固定版本的 jsDelivr；规范本身由 OpenBridge 提供。OpenAPI 覆盖 system 与
OpenAI-compatible HTTP surface，不描述 MCP dual-era transport；MCP 合同见
[网关 API 需求](docs/functional-requirements/gateway-api.md)。仓库中的 [docs/openapi.yaml](docs/openapi.yaml)和
[docs/swagger-ui.html](docs/swagger-ui.html)会被编译进服务，是运行时契约资产，不是派生输出。

## 9. 常见问题

| 现象 | 检查方式 |
|---|---|
| 服务在监听前退出 | 检查 Bootstrap schema、私有文件路径、loopback listener、非零 limit 和 replay/request 上限关系 |
| 请求返回 `401` | 检查是否使用 `users.toml` 中已启用用户的完整 Bearer key |
| 模型不在 `/v1/models` | 检查引用的 credential pool 是否存在有效 source；ChatGPT 还需完成显式登录并重启 |
| 参数在上游调用前被拒绝 | 查看扩展 Models；能力属于所选 Public Model 的固定接口契约 |
| `/healthz` 正常但业务失败 | 健康检查不访问 Provider；继续检查上游 credential、网络和脱敏 request id |
| probe 报 target disabled | 检查 target ID 及其 pool 是否有有效 API key 或 OAuth auth 文件 |
| collector 没有数据 | 检查相应 telemetry signal 是否启用，以及 collector 是否接受 OTLP/HTTP protobuf |
| 端口被占用 | 在 Bootstrap 中改为另一 loopback 地址/端口后重启，不能改为公网监听 |

## 10. 维护者验证

Rust 基线：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

修改 `testdata/` 或 `tools/corpus/` 时追加：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

纯文档维护通常只需要内容、相对链接、锚点和 `git diff --check`。确定性 Rust、fixture 或 loopback 证据不能替代真实 Provider、
外部 SDK、目标 Agent、负载和长期运行验收。

## 11. 进一步阅读

- [文档总索引](docs/README.md)
- [功能需求](docs/functional-requirements/README.md)
- [实施现状](docs/implementation-status/README.md)
- [当前开发焦点](docs/implementation-plans/current-focus.md)
- [外部参考资料](docs/references/README.md)
- [安全配置模板](config/bootstrap.example.toml)

## 开源协议

原创源代码与仓库文档采用 [MIT License](LICENSE)。参考项目只用于协议、行为和实现边界调研；引入外部代码、测试或资源时，必须同时
保留其许可证、版权声明和适用通知。
