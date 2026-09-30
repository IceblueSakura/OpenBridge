# 受控 Provider / SDK / pi 探测

这些是显式授权的开发工具，不是生产重试器、压测器或额外协议框架。默认优先 NVIDIA；Kimi 暂停真实测试，但产品绑定和离线语义测试保留。存在 key、创建计划或历史成功都不授权新的调用。

## 先计划，后执行

全部自动 live 入口共享同一个 run 目录和 SQLite 账本。计划创建及 dry-run 不读取凭据、不启动服务、不联网：

```sh
uv run --project tests/sdk --locked --offline python examples/probe.py plan \
  testdata/runtime/my-run --providers nvidia --limit 16 --tokens 2048 --continue-oracle
uv run --project tests/sdk --locked --offline python examples/probe.py run \
  testdata/runtime/my-run --model nemotron-3-super --cases json --effort none --dry-run
```

目录必须不存在；不覆盖旧计划。`plan.json` 的 hash 与账本绑定，修改预算/目标后不能继续消费原 run。公开 plan view 是副本；进程间 reservation 使用 SQLite 事务。发送前预留额度，已预留或 dispatched 后中断的请求仍计入预算，不能自动退款或重放同一 scenario。计划有效期 24 小时；过期后禁止新发送，但仍可读取报告和完成已预留请求的记录。

授权覆盖这一具体批次后，显式运行：

```sh
cargo build --locked --offline --bin openbridge
uv run --project tests/sdk --locked --offline python examples/probe.py run \
  testdata/runtime/my-run --live --model nemotron-3-super --cases json --effort none
uv run --project tests/sdk --locked --offline python examples/probe.py report testdata/runtime/my-run
```

- Provider 只取固定 catalog 中的 API-key 绑定；不跟随 OAuth 文件，不轮换/改写 key。实际客户端只获得临时 gateway token。
- `--model` 可重复，必须属于计划；`--protocol chat|responses`、`--delivery json|sse` 缩小范围。`--effort none|minimal|medium` 是明确请求控制，不自动改默认。
- cases：`text`、`json`、`tool`、`history`、`length`、`cancel`；`reasoning` 仅用于已选 Luna 的真实 opaque 获取/回放。
- `history` 每个交付四请求，两个实际 lookup 调用/返回；`length` 是 8-token Chat 截断，只有这一场景接受 length；`cancel` 是 Chat SSE 提前关闭和后续普通请求，不证明 Provider 停算/停止计费。
- 默认串行、SDK 零重试，精确限制目标 origin/port/path、model、请求大小、输出 cap 与完整序列化历史。不会因省略 filter 而跳出 run 的模型集合。
- 所有组在 I/O 前登记；未执行的请求显示 `not_run`，中断的 reservation/dispatched 不算通过。HTTP/传输/wire/配置错误停止该目标；工具链失败跳过其依赖轮次。只有计划明确 `--continue-oracle` 时，内容/预期终态失败后才继续独立组。不自动重复失败请求。
- `source_fingerprint` 标记计划创建及各进程预留时的源码/锁文件状态，进程内检测源码变化后拒绝新预留。它不是对正在运行的 binary 的密码学证明；Rust 改动后必须按上述命令重新构建。

## 共用执行层与结果解释

`examples/probe_support/` 分担 catalog、显式 checks、ledger、SDK collectors、raw wire、listener/send/report 生命周期及固定 scenarios。具名入口只选择场景，不各自实现预算、清理或验收。没有动态插件、业务 transformation 或新 task schema。

实际网络 send 强制流式读取，即使请求 JSON，也先检查 raw 字节预算再交给 SDK。SSE 观察有独立 frame/event/text 上限；终态所在 chunk 保留到真实 HTTP EOF 后再交给会在 DONE 停读的 SDK，防止尾随数据被藏起来。错误使观察状态不可恢复。观察器只针对网关输出，不取代 Rust codec 或完整 SSE 标准验证。

结果分列：`sdk_consumed`、`wire_closed`、`terminal`、`content_ok`、`history_ok`。SDK 成功解析不自动证明 wire 闭合；HTTP 200 不代表生成完成；`length` 不转成 stop。严格文本/JSON/工具 oracle 使用显式异常检查，`python -O` 不会绕过。工具结果要求实际调用和关联正确，并满足明确返回值，不用子串出现代替语义正确。stdout 和持久报告不保存正文、参数、reasoning、opaque 或 credential。

`ledger.sqlite3` 是权威账本，`summary.json` 是原子替换的派生视图。状态为 `reserved/dispatched/passed/oracle_failed/failed/cancelled/not_run`；取消是预期测试动作，不是成功生成终态。报表字段使用封闭枚举和有界数值，不能借 arbitrary error/message/header 字段保存私有值。

## 原调用诊断

见 [HTTP guide](http-gateway.md#操作者诊断)。runner 为每个 owned binary 创建新的、私有 `gateway-*.jsonl` 文件，以 `x-openbridge-probe-id = run-id:attempt` 关联同一次请求。诊断在入口认证后才开始，且不向客户端回显。

可观察字段限于最后阶段、完成/中断结果、上游 HTTP、规范化 Retry-After、接收/已 handoff 字节和耗时。`upstream_head_ms`、`first_upstream_bytes_ms` 从认证后的请求处理开始计时，不是 TTFT 或 Provider 纯推理时间；handoff 不等于客户端收到。静态 JSON 的解析可发生于 `terminal`（intake EOF finalize）阶段。

有界队列满、文件预算满、写失败或强杀均可能缺少诊断，缺失必须记为未知，不据此猜测上游状态。原 HTTP 错误映射、取消和交付策略不变；这不是生产可观测性系统，也不因此启用任何自动 retry/backoff。

## 具名入口与库级对照

以下入口保留有意义的场景选择，统一要求 `OPENBRIDGE_PROBE_RUN=<existing run>`：

| 入口 | 显式 gate / 场景 |
|---|---|
| `examples/live_nvidia_probe.py` | `OPENBRIDGE_NVIDIA_PROBE=1`；原有 CASE/DELIVERY/EFFORT filter，仅 NVIDIA |
| `examples/live_provider_matrix.py` | `OPENBRIDGE_PROVIDER_MATRIX=1`；原有 PROVIDERS/PROTOCOL/DELIVERY/CASE filter，默认只取计划的模型 |
| `examples/live_gateway_probe.py` | `OPENBRIDGE_GATEWAY_PROBE=1`；Luna 普通文本/工具，或 REASONING=1 的两轮真实 opaque 对照 |
| `examples/live_probe.rs` | `OPENBRIDGE_PROBE=1`；库级独立链，默认 NVIDIA，过滤只缩小范围，每次调用在共享账本登记和结算 |

Rust 入口在凭据加载前验证计划选择，默认不做目录请求。显式 `OPENBRIDGE_PROBE_LIST_MODELS=1` 的目录查询也占共享 request slot，不将“免费预检”当作预算外 I/O。所有 native 报告位于 run 子目录。

库级 raw capture 默认关闭；仅另行授权的 synthetic forensic 调查才能启用 `OPENBRIDGE_PROBE_CAPTURE=1`。会拒存包含当前 key 的数据并去掉已知 opaque/signature/credential 字段和 error 对象；这不是通用敏感数据脱敏器，也不授权生产内容捕获。redacted capture 不再是原始 wire 或可靠 replay oracle，不进入独立 fixtures。既有 `OPENBRIDGE_PROBE_REPLAY_DIR` 和 `examples/replay_chat.rs` 保持离线，不加载 key；只能使用明确选定、符合其旧格式边界的历史 synthetic capture。

## pi

非私有代码已入仓：`examples/pi_probe.py` / `pi_probe.mjs`。本机交互配置、真实 key 和 auth 文件不入仓；自动 probe 不读取日常配置，而是在 run 私有子目录生成最小配置，禁用资源发现、重试、compaction 和 cache warming，只注册固定 synthetic read。

```sh
uv run --project tests/sdk --locked --offline python examples/pi_probe.py \
  --package /path/to/pi-0.87.1 --model nemotron-3-super --check --invalid-auth
```

`--check` 使用 synthetic key 和拒绝 CONNECT 的 loopback proxy；错误 token 必须 401 且零上游连接，正确 token 准入必须到达拒绝代理。不需要 live run；未指定时使用临时 synthetic 账本。

真实调用需同一授权 run：

```sh
uv run --project tests/sdk --locked --offline python examples/pi_probe.py \
  --package /path/to/pi-0.87.1 --model nemotron-3-super --run testdata/runtime/my-run --live
```

中继在首次 await 前同步预留本地 slot，验证正文后再占共享 slot；不修补实际认证/Content-Type。其端口为动态 loopback，不再固定占用 18080。strict 字面结果和未 trim 的 typed 正文/wire 比较独立于模型的标点或大小写偏好。synthetic read 只是受控工具 fixture，不证明通用文件工具或 TUI 兼容。

## 离线验证

```sh
uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p 'test_*.py'
uv run --project tests/sdk --locked --offline python -O -m unittest discover -s tests/sdk -p 'test_*.py'
node --test tests/sdk/*.test.mjs
cargo test --locked --offline --example live_probe
```

认证/状态/诊断文件、HTTP I/O lifetime 的 owning-layer Rust tests 与固定 SDK loopback 门槛仍见 [development](development.md)。synthetic 429/503、断流、并发预算、source/plan 损坏等证明的是本地边界，不是实际 Provider 的限流概率、长期可用性或推理质量。
