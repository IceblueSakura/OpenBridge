# Mock Server/Client 测试工具现状

操作命令、HTTP 行为、SSE parser、scenario/plan 和 observation 字段见 [Testkit 指南](../../tools/corpus/README.md)；本文件只记录当前实现证据与边界。

## 已实现

`tools/corpus/` 的 Python `0.5.0` 工具现已包含：

- incremental SSE parser；
- canonical case 到 self-contained server scenario/client plan 的编译；
- `asyncio + h11` 单 scenario/有序多-exchange HTTP/1.1 Mock Server；
- 不重试、不依赖 OpenAI SDK 的 Mock Client；
- server/client observation 与敏感 header 脱敏；
- normal terminal、logical EOF、HTTP error、transport abort 和 event cancellation 的观测模型；
- `/health`/`/healthz`、非法 JSON 400、未知 endpoint 404、错误方法 405 和 exchange 耗尽 409；
- Chat/Responses 原生非流式基线，以及 400、401、403、404、422、429、500、502、503、504 HTTP error cases；
- delta-seconds/HTTP-date `Retry-After`、纯文本/损坏 JSON body 和错误状态携带 SSE Content-Type 的边界；
- HTTP status 优先错误分类，避免把 `4xx/5xx + text/event-stream` 误判为 SSE EOF/terminal。

运行时 schema 位于：

- `testdata/schemas/server-scenario.schema.json`；
- `testdata/schemas/server-suite.schema.json`；
- `testdata/schemas/client-plan.schema.json`；
- `testdata/schemas/observation.schema.json`；
- `testdata/schemas/server-run-observation.schema.json`。

`testdata/runtime/` 保存可重建 scenario、plan、ready state 和 observation，不进入 Git 或 corpus ZIP。

当前 testkit 提供离线 mock，不提供真实上游 proxy、credential 注入或安全响应 header 白名单能力。

## 已验证

2026-07-26 在 Windows、Python 3.12、uv 管理环境执行：

```powershell
uv run --project tools/corpus pytest tools/corpus/tests -q
uv run --project tools/corpus corpus --root testdata lint
```

结果：

- 26 passed；
- corpus lint passed；
- incremental parser 逐一消费 306 个生成 wire variants；
- 类级 loopback 覆盖 stream terminal、HTTP 503、输出后 abort 和事件后取消；
- 进程级 CLI loopback 完成 `build-server-scenario -> mock-server -> build-client-plan -> mock-client`；
- CLI observation 显示 `response_completed`、`/v1/responses` 和双方 terminal；
- 多-exchange CLI 验证显示 health `ok`、两个 exchange、HTTP 429 与 `Retry-After: 1`；
- 非法 JSON 和未知 endpoint 不会消耗待执行 exchange。
- HTTP error matrix loopback 验证 10 个新增 case 的 status、headers 与原始 body。
- 移除 Rust fixture binary 后，`cargo test --all-targets` 通过 53 个 Rust tests，SDK compatibility 的 1 个外部依赖测试仍按原设计 ignored。

## 尚未证明

- 未加载或启动 OpenBridge；
- 未验证任何 OpenBridge Chat/Responses 转换、routing、retry 或 fallback；
- 未验证 TCP packet 边界；
- 未验证 TLS、HTTP/2、并发、背压、负载或真实 SDK/Provider；
- suite 仍是启动前静态编译的有序队列，不支持运行时控制面或并发调度。
