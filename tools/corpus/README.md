# OpenBridge Testkit

`tools/corpus/` 是与 OpenBridge runtime 解耦的 Python 工具包。它管理 [../../testdata/](../../testdata/README.md) 的
canonical corpus，并提供增量 SSE parser、HTTP/1.1 Mock Server、Mock Client、scenario/plan 编译、observation 输出与协议无关的
function-tool semantic trace 判定。

它服务于未来的黑盒链路：

```text
Mock Client  ->  future SUT (例如 OpenBridge)  ->  Mock Server
```

本工具本身不会加载 OpenBridge 配置、启动 OpenBridge、读取 credential、调用真实网络 Provider 或自动重试。它只提供确定的两端行为与可比较的观察记录。

## 安装与日常验证

前置条件：已安装 [`uv`](https://docs.astral.sh/uv/)，并使用 Python 3.12 或更高版本。所有命令从仓库根目录执行：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

包名为 `openbridge-testkit`，CLI 有两个等价入口：`corpus` 与 `obtest`。下面都使用 `corpus`。

## 命令参考

| 命令                    | 输入                              | 输出                          | 用途                                                             |
|-------------------------|-----------------------------------|-------------------------------|------------------------------------------------------------------|
| `lint`                  | canonical corpus                  | stdout/exit code              | 校验 schema、路径、artifact 组合、SSE、provenance 与 secret scan |
| `generate`              | case SSE + recipe + seed          | `testdata/generated/`         | 生成确定的 Base64 wire chunks 与 manifest                        |
| `report`                | canonical corpus                  | stdout 或 `testdata/reports/` | 输出覆盖、status、来源与缺口统计                                 |
| `pack`                  | canonical corpus                  | `testdata/dist/`              | 构建 deterministic ZIP 和 SHA-256 sidecar                        |
| `build-server-scenario` | 一个有上游 attempt 的 case        | `testdata/runtime/`           | 编译一个自包含上游 HTTP scenario                                 |
| `build-server-suite`    | 有序 case 列表                    | `testdata/runtime/`           | 编译按请求顺序消费的多 exchange suite                            |
| `build-client-plan`     | 一个 case + SUT/base URL          | `testdata/runtime/`           | 编译 Mock Client 请求计划                                        |
| `build-semantic-plan`   | semantic case + 可选 length/position | `testdata/runtime/`        | 编译零网络 function/context/structured execution plan           |
| `mock-server`           | scenario 或 suite JSON            | ready/observation JSON        | 启动 HTTP/1.1 upstream fixture                                   |
| `mock-client`           | client plan JSON                  | observation JSON              | 发送一次请求并记录结果                                           |
| `verify-observations`   | case + client/server observations | stdout/exit code              | 用 canonical oracle 判定单 case 结果                             |
| `verify-semantic-trace` | semantic case + normalized trace  | stdout/exit code              | 判定工具、context 固定事实与 strict structured output            |

默认 corpus root 是 `./testdata`。传递 `--root testdata` 可使脚本和 CI 的工作目录显式。

### Corpus 管理命令

```powershell
uv run --project tools/corpus corpus --root testdata lint
uv run --project tools/corpus corpus --root testdata generate --seed 20260726
uv run --project tools/corpus corpus --root testdata report --output testdata/reports/coverage.json
uv run --project tools/corpus corpus --root testdata pack --output testdata/dist/openbridge-protocol-corpus-0.8.0.zip
```

`generate`、`report`、`pack` 的 `--output` 受限于 `generated/`、`reports/`、`dist/`。scenario、plan、ready state 和 observation
的输出受限于 `runtime/`。这是故意的防护：工具不能清理或写入 canonical case 目录。

## 从 case 到 HTTP loopback

以下示例不经过 OpenBridge，直接验证 Mock Client 与 Mock Server 对一个 Responses SSE case 的闭环。

第一个终端先编译并启动 Server：

```powershell
uv run --project tools/corpus corpus --root testdata build-server-scenario `
  --case responses_native.sse_framing `
  --variant event_pairs

uv run --project tools/corpus corpus --root testdata mock-server `
  --scenario testdata/runtime/responses_native.sse_framing.server-scenario.json `
  --ready-file testdata/runtime/server-ready.json `
  --observation testdata/runtime/server-observation.json
```

Server 立即把实际端口写入 `server-ready.json`，然后等待一个 scenario 被消费。读取其中的 `base_url` 后，在第二个终端构建并运行
Client：

```powershell
uv run --project tools/corpus corpus --root testdata build-client-plan `
  --case responses_native.sse_framing `
  --base-url http://127.0.0.1:<port>

uv run --project tools/corpus corpus --root testdata mock-client `
  --plan testdata/runtime/responses_native.sse_framing.client-plan.json `
  --observation testdata/runtime/client-observation.json
```

完成后，两个 terminal 都会写入 observation。它们是后续 runner 的输入证据，不是产品断言结果。

### 有序多 exchange suite

suite 只允许按声明顺序认领 exchange，适合未来 runner 验证有界重试或 fallback 的请求序列：

```powershell
uv run --project tools/corpus corpus --root testdata build-server-suite `
  --suite-id retry-sequence `
  --case responses_native.rate_limit.http_date.non_stream `
  --case responses_native.text.non_stream

uv run --project tools/corpus corpus --root testdata mock-server `
  --scenario testdata/runtime/retry-sequence.server-suite.json `
  --ready-file testdata/runtime/server-ready.json `
  --observation testdata/runtime/server-observation.json
```

每个普通 POST 原子地消费一个 exchange；`/health` 与 `/healthz`、非法 JSON、未知 endpoint 和错误 HTTP method 不会消费。没有剩余
exchange 时，Server 返回 `409`。suite 目前是静态、有序、单进程队列，不是并发调度器。

## Scenario 与 Client plan

### Server scenario

`build-server-scenario` 从 case 的 `expected_upstream_request`、`upstream_response` 或 `upstream_stream` 编译 JSON。其
response 包含：

- status、headers、完整 wire 的 SHA-256；
- Base64 chunk 数组；
- `chunk_delay_ms`；
- `termination`：`complete` 正常完成 HTTP message，或 `abort` 在写出 chunks 后异常断连；
- `abort_delay_ms`。

`--variant canonical` 使用原始 artifact bytes；其他 variant 需要先运行 `generate`。可用 `--chunk-delay-ms` 让
cancellation/abort 演示更容易复现，但它不模拟吞吐、背压或真实 TCP packet。

### Client plan

`build-client-plan` 从 case 的 `client_request` 编译 HTTP URL、method、headers、Base64 body、SHA-256、stream 标记、timeout 与可选
`cancel_after_event`。Client 只支持绝对 `http://` URL；目前不支持 HTTPS、HTTP/2、proxy 或 WebSocket。

所有 plan 与 scenario 都经过 `testdata/schemas/` 的 JSON Schema 校验。直接编辑 runtime JSON 时，也应通过对应命令重新验证，而不是将其作为新的
canonical oracle。

## Mock Server 行为

Mock Server 基于 `asyncio + h11`，默认只监听 `127.0.0.1` 和随机可用端口。

| 请求条件                                        | 响应                                           | 是否消费 exchange |
|-------------------------------------------------|------------------------------------------------|-------------------|
| `GET /health` 或 `/healthz`                     | `200`，body 含 `status` 与 `pending_exchanges` | 否                |
| 非 POST 业务请求                                | `405 method_not_allowed` JSON error            | 否                |
| 非 `/v1/chat/completions`、`/v1/responses` 路径 | `404 unknown_fixture_endpoint` JSON error      | 否                |
| 非法 JSON body                                  | `400 invalid_json` JSON error                  | 否                |
| 无剩余 suite exchange                           | `409 no_pending_exchange` JSON error           | 否                |
| 有效业务请求                                    | 按下一个 scenario 写 status、headers 与 chunks | 是                |

Server 不会在收包过程中主动比较 request 与 `expected_request`；它会在 observation 中记录 method、target、脱敏 headers、raw
body、JSON（若可解析）、hash、response status、终止方式、SSE terminal 和 timing。单 case 可在运行完成后交给
`verify-observations` 判定；多 attempt/retry/fallback runner 仍需负责序列编排。

被记录时会脱敏 `authorization`、`cookie`、`proxy-authorization`、`set-cookie` 与 `x-api-key` 的值。

## Mock Client 行为

Mock Client 基于 `asyncio + h11`，每个 plan 只发送一次请求：

- 不使用 OpenAI SDK；
- 不自动 retry、fallback 或解析 `Retry-After`；
- 对 SSE 使用增量 parser，不假设 `read()` 与 chunk 边界一致；
- 在第 N 个逻辑 SSE event 后可按 plan 中的 `cancel_after_event` 断开；
- 保存 raw response chunks、完整 body、JSON（若 `application/json` 且可解析）、headers、SSE events、terminal、结束分类和 timing；
- 同样对敏感 response headers 脱敏。

结束分类如下：

| `end`             | 含义                                        |
|-------------------|---------------------------------------------|
| `response`        | 已正常完成的非 SSE 成功响应                 |
| `error_response`  | 已完成 HTTP message 且 status 为 `4xx/5xx`  |
| `terminal`        | SSE 正常出现协议 terminal                   |
| `eof`             | SSE HTTP message 完成但没有 terminal        |
| `transport_error` | 未完整完成 HTTP message、连接异常或 timeout |
| `cancelled`       | Mock Client 按计划主动终止连接              |

HTTP status 的错误分类优先于 Content-Type：即使上游错误地将 `500` 标为 `text/event-stream`，Client 仍记录 `error_response`
，不会将其伪装为 SSE EOF 或 terminal。

## SSE parser 的边界

增量 parser 直接消费 bytes，支持 LF、CRLF、CR、comment、多个 `data:` 行、一个 read 内多个 event、UTF-8 跨 chunk、`[DONE]` 和
Responses terminal。它同时保留 SSE `event:` field 与 JSON payload 的 `type`；两者冲突时记录 `type_conflict`。

EOF 不会派发缺少最后空行的半个 event。SSE `error`、Responses `failed`/`incomplete` 是逻辑 terminal；HTTP
error、EOF、transport abort 与 client cancellation 是不同层次的结果。

## Observation 与后续 runner

单 exchange observation 至少含有：

- `schema_version`、`role`、`case_id`；
- `body_base64`、`body_sha256`；
- `end`、`error` 和 `timing`；
- HTTP response/request envelope；
- 对 SSE，解析后的 event、terminal 与 conflict 信息。

多 exchange Server 的 observation 用 `mock_server_run` 包装并按 suite 顺序保存。Schema 有意允许单 exchange observation
附加字段，使工具能增加非破坏性的诊断；runner 应只依赖明确文档化或 schema 定义的稳定字段。

### Semantic execution plan 与 trace 判定

`build-semantic-plan` 不启动 OpenBridge、不读取 credential，也不选择 model/Provider。function 与 structured task 原样编译为 runtime
plan；context task 根据 case seed、声明的 UTF-8 byte 长度和 start/middle/end 位置生成精确长度 prompt：

```powershell
uv run --project tools/corpus corpus --root testdata build-semantic-plan `
  --case context.literal_retrieval `
  --target-bytes 16384 `
  --placement middle
```

byte 是确定性生成控制量，不是 token 声明；真实 runner 必须单独记录实际 input token usage。

semantic verifier 消费已经规范化的 trace，而不解析 Chat/Responses wire。trace 只包含按时间排列的
`assistant_tool_call`、`tool_result`、`assistant_message` event；调用参数必须已经是 JSON object。Native Chat、Native Responses、
Chat → Responses 或 Responses → Chat runner 都可映射到同一份 semantic case：

```powershell
uv run --project tools/corpus corpus --root testdata verify-semantic-trace `
  --case function.parallel_independent `
  --trace testdata/semantic-cases/function/function.parallel_independent/reference-trace.json
```

命令先校验 trace schema 和 call/result identity，再检查 function 参数 schema、调用集合、预置 tool result、context 固定事实，
以及 structured assistant text 的 JSON parse/schema。成功返回 `0`；不匹配返回 `1`，且诊断不回显正文。schema 不合法、case
不存在或 JSON 损坏也返回 `1`。

testkit 不负责从任一协议 envelope 自动提取 trace，也不调用模型生成 trace；这两个步骤属于下一阶段显式接入的 runner。

### 单 case observation 判定

当 SUT 两侧的 observation 已生成后，可按 case oracle 进行一次确定性判定：

```powershell
uv run --project tools/corpus corpus --root testdata verify-observations `
  --case responses_native.text.non_stream `
  --client-observation testdata/runtime/client-observation.json `
  --server-observation testdata/runtime/server-observation.json
```

`upstream_attempts = 0` 的 preflight reject case 省略 `--server-observation`。命令校验 observation schema 与 body
hash，然后比较 case identity、上下游请求 path、JSON 或 SSE body、HTTP status、结束分类、terminal 和 case 声明的下游 response
headers。通过返回 `0`；失败返回 `1`，只输出字段路径或摘要，不回显完整正文。

该命令只判定零次或单次上游 attempt，且不负责启动 OpenBridge、Mock Server 或 Mock Client。它不判定 route 选择、retry/fallback
序列、时序窗口、SDK/CLI 行为或真实 Provider 兼容性。

典型 process runner 应进行：

```text
canonical client request -> SUT -> Mock Server
       |                     |        |
       |                     |        +-> 比较 expected_upstream_request
       |                     +-> 记录 attempt/retry/fallback 决策
       +-> Mock Client <-----+-> 比较 expected client body/SSE/terminal
```

testkit 已能对单 case 的最终 observations 做 canonical comparison，但不负责进程编排、多 attempt 序列或 SUT 产品策略推断。

## 开发与变更规则

- 先更新 canonical case、schema 或 testkit test，再改工具实现；
- runtime JSON、生成 variants、coverage report 和 ZIP 都是派生物，不提交；
- 新增 response/observation 字段时先评估 schema compatibility；破坏性 schema 改动必须升级 schema version；
- 为新 HTTP/SSE 分类补 unit/loopback 测试，覆盖 Server 与 Client 两端；
- 为新 function/context/structured 语义先补 semantic case、reference trace 与 verifier 负例；协议特定字段仍放在 wire case；
- 不在 corpus、plan、scenario 或 observation 中记录真实 credential；
- 不把 tool loopback 结果描述为 OpenBridge、SDK 或真实 Provider 兼容证明。

完整数据模型、版本和 release 规则见 [../../testdata/README.md](../../testdata/README.md)。
