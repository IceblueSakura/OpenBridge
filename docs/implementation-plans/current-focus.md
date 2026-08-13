# 当前开发焦点

## 状态

**已完成并验证。** 认证后的四类下游 HTTP 内容快照已迁移到独立 JSONL 文件；实现事实与验证证据已同步到
[OpenTelemetry 遥测](../implementation-status/telemetry-metrics.md)。当前无进行中的行为。

## 目标

将认证后的四类下游 HTTP 内容快照——request headers、request body、response headers、response body——从 stdout 文本事件迁移到独立、可分析的 JSON Lines 文件。普通运行日志继续输出到 stdout/journald。

## 可观察行为

1. Bootstrap 中启用任一 HTTP 内容日志开关时，必须同时提供绝对日志目录；目录无法创建、今日文件无法打开或不可写时，OpenBridge 在监听前启动失败。
2. 每个 snapshot 写成一行独立 JSON，schema version 固定；正文换行不得破坏 JSONL 记录边界。
3. 每条记录包含 UTC 时间、request id、snapshot kind、HTTP 边界字段和完整性元数据：
   - request headers：method、path、脱敏后的重复 header values；
   - response headers：status、脱敏后的重复 header values；
   - request/response body：`body_base64`、合法 UTF-8 时的 `body_text`、captured/observed bytes、complete、truncated。
4. 文件按 UTC 日期滚动为 `http-YYYY-MM-DD.jsonl`；OpenBridge 不自动删除历史文件。
5. snapshot 进入专用写线程的有界队列。短暂拥塞允许有限等待；队列持续满、运行时写入失败或滚动失败时，产生限频普通诊断并丢弃受影响 snapshot，但不得改变 HTTP/SSE、retry、fallback、取消或 Provider 结果。
6. 正常关闭时在有界时间内停止接收、排空队列并 flush；未完成 drain 或 flush 必须产生明确诊断。该保证不等同于逐记录 `fsync`，进程崩溃或主机断电仍可能丢失尚未持久化的数据。
7. 现有认证后边界、header 强制脱敏、body 大小上限、每方向最多一个 terminal snapshot 与 OTLP exclusion 保持不变。

## 配置合同

`[logging]` 保留四个独立布尔开关，并增加：

```toml
[logging]
http_jsonl_directory = "/var/lib/openbridge/http-logs"
request_headers = true
request_body = true
response_headers = true
response_body = true
```

- 四个开关均为 `false` 时，目录可以省略且不启动 writer。
- 任一开关为 `true` 时，目录必须存在或可安全创建，并且必须是绝对路径。
- 严格 schema 继续拒绝未知字段；不增加环境变量、相对路径 fallback 或兼容 alias。
- 随附的两个开发 Bootstrap profile 必须继续解析为相同配置，并为新增 assignment 保留紧邻的英文运行效果注释。

## JSONL schema v1

共同字段：

```json
{
  "schema_version": 1,
  "timestamp": "2026-08-13T05:55:00.000000000Z",
  "request_id": "...",
  "kind": "request_headers | request_body | response_headers | response_body"
}
```

类型字段：

- `request_headers`：`method`、`path`、`headers`；
- `response_headers`：`status`、`headers`；
- body：`body_base64`、可选 `body_text`、`captured_bytes`、`observed_bytes`、`complete`、`truncated`。

`headers` 使用保留重复值的对象数组，而不是会丢失顺序或重复项的 JSON object。无效 UTF-8 正文只保留可逆的 Base64，不生成 lossy text。

## 范围

- `src/config/`：严格解析目录、条件必填和绝对路径合同。
- `src/observability/`：定义 JSONL record、header 脱敏、writer runtime、队列、UTC 日滚动、flush/shutdown 和限频故障诊断。
- `src/main.rs`：在 listener 前初始化 writer，在 telemetry shutdown 前执行有界 drain；普通 formatter 仍只写 stdout。
- `src/ingress/router.rs`、`src/ingress/lifecycle.rs`：向 snapshot 传递 request id，并保持现有透明 body lifecycle。
- `config/bootstrap.toml`、`config/bootstrap.example.toml`：加入安全的开发日志目录。
- 对应 requirements、implementation status、README/config 文档与确定性测试同步更新。

## 非目标

- 不记录原始 Provider 上游 wire。
- 不把普通 tracing、metrics 或 OTLP spans 写入该 JSONL。
- 不自动压缩、上传、索引或删除日志。
- 不内置 SQLite、DuckDB、dashboard 或查询 API。
- 不提供关闭 header 脱敏的开关。
- 不承诺审计级零丢失、逐记录 `fsync`、崩溃恢复或多进程并发写同一目录。
- 本阶段不修改 NixOS/systemd 部署；部署目录、权限和备份策略在代码合同验证后单独处理。

## 实施任务

1. **先建立失败合同测试**
   - 配置测试覆盖：启用时缺目录、相对目录、未知字段拒绝；全部禁用时允许省略；两个示例配置保持相等。
   - JSONL 测试覆盖：四种 record 可逐行解析、正文换行不拆行、重复 headers 保留、敏感值不出现、无效 UTF-8 可由 Base64 完整还原。
   - writer 测试覆盖：UTC 跨日滚动、队列拥塞/写故障不改变模拟业务结果、关闭 drain/flush、有界故障诊断。
2. **实现配置值对象和启动检查**
   - 在配置域解析条件必填的绝对目录。
   - writer 初始化时安全创建目录和当日文件；Unix 新建目录/文件分别使用 owner-only 权限，并在监听前验证写入边界。
3. **实现独立 JSONL runtime**
   - 使用专用线程和固定容量有界队列；生产请求只提交 owned record。
   - 使用短超时 enqueue；满队列或 writer unhealthy 时丢弃并通过普通 tracing 输出限频、无正文诊断。
   - 每条 JSON 序列化成功后追加换行；按 UTC 日期切换文件；正常关闭排空并 flush。
4. **替换四类 stdout 内容事件**
   - `http_logging.rs` 不再调用 `tracing::info!` 输出 headers/body；改为构造 typed record 并送入 JSONL runtime。
   - `RequestObservation` 持有 writer handle 和稳定 request id；body wrappers 继续只在 EOF、error 或 drop 产生一次 snapshot。
   - 普通 request/attempt lifecycle tracing 保持不变。
5. **同步合同文档与开发配置**
   - 更新观测需求、配置所有权、实施状态、根 README/config 示例和 `AGENTS.md` 中已变更的 sink 边界。
   - 明确文件可能含业务正文、无自动保留清理、生产部署需要受限目录及外部生命周期管理。
6. **验证并收敛**
   - focused tests 先行，再执行完整 Rust baseline；验证 JSONL 实际文件可被逐行解析且 stdout 中不存在四类内容 sentinel。
   - 完成并记录确认事实后，将本文件恢复为无进行中行为。

## 验证

```text
cargo test --locked --test config_contract
cargo test --locked --test example_config
cargo test --locked --test observability_contract
cargo test --locked --test otlp_trace_contract
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

另执行一个临时目录集成测试：发送包含多行 UTF-8 和无效 UTF-8 synthetic body 的认证请求，逐行用 `serde_json` 解析生成文件，校验四类 kind、request id 关联、Base64 往返、redaction、完整性字段、stdout exclusion 和关闭后的文件可读性。不得使用真实 credential、私人正文或真实 Provider。

## 风险与回滚

- **敏感正文落盘**：维持显式 opt-in、启动警告、owner-only 新文件权限和 header 强制脱敏；生产所有者应关闭不需要的 body 开关。
- **磁盘容量耗尽**：不自动删除符合已选策略，但运行时写失败必须限频告警；容量监控和外部保留策略属于部署责任。
- **请求路径抖动**：有界队列只允许短等待；持续拥塞转为明确丢弃而非无限阻塞。
- **同日文件被外部移动/替换**：writer 持有已打开文件直到 UTC 换日；本阶段不承诺与外部 logrotate 协同。
- **回滚**：关闭四个内容开关即可不启动 writer；代码回滚不改变 HTTP API、Provider 路由或 OTLP schema。
