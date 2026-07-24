# upstream-fixture-server

用于 OpenBridge 原生路径验收的本地上游服务。它不是生产代理，也不实现 Chat 与 Responses 的协议转换。

## 模式

- `mock`（默认）：离线返回确定性 Chat/Responses JSON、SSE 与 429 fixture。
- `proxy`：将 `/v1/chat/completions` 和 `/v1/responses` 转发到明确配置的真实上游；仅在请求缺少 `model` 时补入可选默认模型，并以 `.env` 中的 API key 替换入站认证。

服务只监听 loopback 地址。`proxy` 模式不记录 request body、响应 body 或 API key；它只复制 SDK 需要的安全响应头和流式 body。上游本身不支持的 endpoint 会将原始 HTTP 状态和 body 返回给调用者，测试服务不会暗中执行 Chat ↔ Responses bridge。

## 配置

在本目录中复制 [`.env.example`](.env.example) 为 `.env`，或在进程环境中设置同名变量（进程环境优先）。程序始终加载 `tools/upstream-fixture-server/.env`，与启动时的工作目录无关；`.env` 已被 git 忽略。

| 变量 | 默认值 | 说明 |
|---|---|---|
| `UPSTREAM_FIXTURE_MODE` | `mock` | `mock` 或 `proxy`。 |
| `UPSTREAM_FIXTURE_LISTEN` | `127.0.0.1:4010` | 仅允许 loopback socket address。 |
| `UPSTREAM_FIXTURE_API_BASE` | — | `proxy` 必填，例如 `https://api.openai.com/v1/`。 |
| `UPSTREAM_FIXTURE_API_KEY` | — | `proxy` 必填；仅在出站 `Authorization: Bearer` header 中使用。 |
| `UPSTREAM_FIXTURE_MODEL` | — | 可选默认模型；只在请求缺少 `model` 时补入，显式模型不覆盖。 |
| `UPSTREAM_FIXTURE_TIMEOUT_MS` | `120000` | proxy 请求超时。 |

## 运行

```powershell
cargo run --bin upstream-fixture-server

# 使用真实上游：先在 .env 设置 MODE、API_BASE 和 API_KEY。
cargo run --bin upstream-fixture-server
```

健康检查：`GET http://127.0.0.1:4010/health`。

`UPSTREAM_FIXTURE_MODEL` 适合直接向 fixture server 发送的最小上游探测。它不能替代 OpenBridge deployment 的 `upstream_model`：正常 Native Path 验收仍应由 deployment 明确选择模型，fixture server 不会覆盖该请求已经携带的 `model`。

## proxy 边界

- 仅转发 JSON body；调用方的 `Authorization` 和其他入站 header 不会透传。真实上游只接收本地 `.env` 提供的 Bearer credential。
- 仅返回 `Content-Type`、retry、request-id 和 rate-limit 等白名单响应 header；不会转发 cookie、连接控制或任意未知 header。
- URL 不允许 query/fragment，且只允许 `http`/`https`；listener 只允许 loopback。
- 该工具不持久化或记录 API key、request body、response body。它用于验收，不应部署为共享代理。

## mock fixture

两个 endpoint 均支持 `stream: false/true`：

- `POST /v1/chat/completions`
- `POST /v1/responses`

普通请求返回确定性文本；请求体任意字符串字段包含 `__fixture:rate-limit__` 时返回 HTTP 429 和 OpenAI 风格 error body。它适合当前 M1-S 的 SDK 与 forwarding 验证；tool loop、EOF、partial stream、cancel 等场景仍由更细粒度的 Rust contract fixture 覆盖，后续可在不改变 proxy 模式语义的前提下补充 scenario。
