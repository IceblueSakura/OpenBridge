# 测试补全计划

## 1. 状态与使用方式

**候选计划，不是当前实施焦点。** 本文件整理当前 checkout 的高价值测试缺口、补全顺序和验证边界，
不表示任一条目已经获准实施。开始具体工作前，必须只选择一个可观察行为，将对应内容写入
[当前开发焦点](current-focus.md)，完成测试与最小实现后再更新实施现状并清空当前焦点。

本计划只补强现有产品行为的确定性证据，不改变产品范围、Provider 能力声明、Protocol Bridge 语义或
默认验收层。当前实现事实仍以[当前实现总览](../implementation-status/current-implementation.md)和
[协议测试语料与工具现状](../implementation-status/protocol-test-corpus.md)为准。

## 2. 当前测试基线

本快照于 2026-08-07 在第一阶段低质量测试清理后，从当前 checkout 的测试收集结果和 canonical catalog 得到：

| 测试资产               | 当前数量 | 主要责任                                                                  |
|------------------------|---------:|---------------------------------------------------------------------------|
| Rust 源码内单元测试    |       55 | 局部算法、状态、边界类型和安全不变量                                      |
| Rust 默认集成测试      |      221 | registry、routing、Provider、HTTP/SSE、retry/fallback、取消和 Bridge      |
| Rust ignored 客户端测试 |        2 | 独立 Embeddings Python client 与 OpenAI Python/Node SDK opt-in loopback    |
| Python testkit 测试    |       36 | corpus、SSE parser、Mock Server/Client、observation verifier、生成与打包  |
| Canonical corpus       | 45 cases | Chat/Responses、Bridge、HTTP error、SSE terminal、transport 与取消 oracle |
| 默认 SSE wire variants |      306 | Python 所有的确定性 byte fragmentation 覆盖                               |

Rust 共收集到 276 个默认测试和 2 个 ignored 客户端测试。第一阶段删除了 7 个只锁定私有模块位置、指针身份、普通容器操作或
重复已存在强契约的测试，并把混合的 reasoning-output 测试收窄到唯一未重复的 MiMo Native 行为。静态 case-id 引用扫描显示，45 个 canonical case 中有 26 个被 Rust
测试源码直接引用，19 个未被直接引用。这个数字只表示 fixture 与 Rust 测试的直接连接， 不等于 19 个行为完全没有 synthetic
contract test，也不构成行覆盖率或分支覆盖率结论。

当前环境没有安装 `cargo-llvm-cov`。本计划以可观察行为、故障风险和证据断层排序，不设任意覆盖率百分比目标。

## 3. 补全目标

1. 让已有 canonical oracle 更完整地经过生产 Router、Provider adapter 和 HTTP/SSE 生命周期，而不只停留在 corpus lint、独立
   testkit 或局部状态机回放。
2. 优先保护首输出 commit point、EOF、transport abort、下游取消、HTTP error 分类和 terminal 等高风险边界。
3. 为每个已编译 Provider 保留与自身 wire profile 对应的确定性契约，避免共享 OpenAI-compatible 代码掩盖 Provider 差异。
4. 补强共享 credential cursor、member/fault cooldown、attempt budget 与取消之间的并发不变量。
5. 对所有已认证请求终态建立可观测性对称断言，确保只完成一次观测且不保存正文或 credential。
6. 保持默认反馈路径有界：先运行 focused Rust test，再运行 Rust baseline；只在修改 `testdata/` 或
   `tools/corpus/` 时追加 Python baseline。

## 4. 测试所有权

| 行为                                                                       | 首选所有者             | 约束                                                 |
|----------------------------------------------------------------------------|------------------------|------------------------------------------------------|
| Route、retry/fallback、cooldown、取消、Bridge 和 observability             | Rust tests             | 不在 Python 中复制 OpenBridge 策略                   |
| Canonical schema、provenance、secret scan、生成、打包和 byte fragmentation | Python tests           | 不隐式启动 OpenBridge 或调用真实 Provider            |
| Canonical wire 对生产 Router 的回放                                        | Rust integration tests | 只读复用 fixture；逐项扩展现有 process replay helper |
| 当前外部 SDK 解析行为                                                      | ignored SDK tests      | 显式运行并记录实际版本和安装来源                     |
| 真实 Provider 差异                                                         | 独立 acceptance        | 不进入默认确定性测试，也不替代 fixture regression    |

不要为了提高数字在 Rust 中复制 306 个 Python fragmentation variants，也不要在 Python testkit 中实现 Route、 retry、fallback
或 credential rotation。

## 5. 优先级

| 优先级 | 补全方向                            | 当前缺口                                                                                  | 完成标志                                                                                    |
|--------|-------------------------------------|-------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| P0     | Canonical case 经过生产 Router 回放 | process replay 当前只覆盖一个 Responses 429 case                                          | 高风险 transport/fault case 能比较实际上游请求、下游响应、terminal、attempt 和 commit point |
| P0     | Streaming 失败与取消                | corpus 的 5 个 transport case 均未被 Rust 测试源码直接引用                                | EOF、abort、cancel 在输出前后边界均不产生非法 retry/fallback 或伪造 terminal                |
| P1     | Provider wire profile               | Provider contract 数量较多，但每个 Provider 的 path、terminal、header 和 error 覆盖不对称 | 每个已编译 Provider 都有与声明能力匹配的表驱动契约                                          |
| P1     | credential/cooldown 并发            | 现有覆盖以顺序场景为主                                                                    | 并发请求下 cursor、cooldown、generation、attempt budget 和取消仍保持确定性不变量            |
| P1     | observability 终态矩阵              | 已有成功、失败、EOF、取消测试，但协议和多 attempt 场景不完全对称                          | 每个已支持终态只记录一次，计数正确，诊断不含正文和 credential                               |
| P2     | SDK 测试可诊断性                    | 一个 ignored 测试同时执行 Python、Node、Chat、Responses 和 tool 场景                      | SDK、协议和行为失败可独立定位，仍保持 opt-in                                                |

## 6. 候选短周期行为

以下条目是候选工作，不应在一次改动中全部实施。每次只将一个条目转入 `current-focus.md`。

### TC-01：HTTP error 优先于 SSE Content-Type 分类

- 可观察行为：上游返回 HTTP `>=400` 且错误地携带 `text/event-stream` 时，生产 Router 按 HTTP error 处理，保留允许传递的
  status、body 和 header，不进入 SSE terminal/EOF 状态机。
- 第一条测试：在 `tests/process_replay_contract.rs` 回放
  `responses_native.http_error.sse_content_type`，比较上游 request、下游 error response、零 SSE terminal 和 单次 attempt。
- 最小边界：只扩展现有 process replay helper 对可配置 status、headers 和 response body 的支持。
- 不做：不新增通用 CLI，不修改 Python testkit，不解析 Provider 私有 error body。
- focused validation：`cargo test --locked --test process_replay_contract <test-name>`。

### TC-02：首输出后 transport abort 不触发第二次 attempt

- 可观察行为：上游已经产生下游可见输出后 transport 异常断开，Router 关闭当前流，不 retry、不 fallback， 不伪造成功或失败
  terminal。
- 第一条测试：回放 `responses_native.transport_error.after_output`，断言已有输出、无 terminal、attempt 为 1， observability
  记录一次 stream failure。
- 最小边界：为 loopback mock response 增加明确的 abort 结束方式和可等待的首输出事件。
- 不做：不模拟吞吐、背压、TLS、HTTP/2 或真实 TCP packet 边界。
- 并发要求：使用 readiness/event 和有界 timeout，不添加 sleep。

### TC-03：下游在逻辑事件后取消会停止上游工作

- 可观察行为：下游在声明的 SSE event 后断开，当前上游 body 被取消，退避和后续 attempt 不再启动， observability 只记录一次
  cancellation。
- 第一条测试：回放 `responses_native.cancel.after_output`，在确定的 event 边界 drop 下游 body，并观察 mock upstream drop
  signal、attempt 数和终态计数。
- 最小边界：复用现有 cancellation transport 与 process replay 生命周期，不引入后台控制平面。
- 不做：不声明负载、公平性或长期连接稳定性。

### TC-04：Native Chat/Responses canonical 成功路径经过生产 Router

- 可观察行为：两个下游 endpoint 的 native non-stream 和代表性 SSE 成功 case 均保持 canonical 上游 request、 下游
  envelope、safe headers 和明确 terminal。
- 初始 case：先选择 `responses_native.text.non_stream`；通过后再分别选择
  `chat_native.text.non_stream`、`responses_native.sse_framing` 和 `chat_native.sse_framing`，每次仍作为独立焦点。
- 抽取规则：至少两个 case 使用相同 setup 后，才把重复逻辑收敛为表驱动 helper。
- 不做：不让一个大参数化测试掩盖 Chat/Responses 的失败定位。

### TC-05：Provider wire profile 对称契约

- 可观察行为：每个已编译 Provider 的 adapter 只生成受信相对 path、正确 upstream model、所属 credential、 普通 header policy
  和已声明 terminal profile；未配置 wire shape fail closed。
- 执行顺序：优先补 OpenRouter data-only terminal 与 HTTP error 组合，再补 LongCat、MiMo、DeepSeek 和 OpenAI。
- 第一条测试：每次只选择一个 Provider 的一个缺口，在 `provider_contract.rs` 或
  `provider_boundary_contract.rs` 增加表项或独立测试。
- 不做：fixture 不能被描述为真实 Provider 可用性；没有证据时不扩大 capability。

### TC-06：共享 credential 与 cooldown 的确定性并发

- 可观察行为：并发请求共享 round-robin cursor，但不会突破请求级/candidate attempt 上限；一次 member 429 只冷却对应
  generation，成功 peer 不会清除其他 member 状态，取消不会借用下一个 credential。
- 第一条测试：两个请求通过 barrier 同时选择同一 pool，显式控制一个 429、一个成功，并验证选择序列和 后续 cooldown 行为。
- 后续独立行为：generation 切换隔离旧状态、fault domain 并发失败、取消与 backoff 竞争。
- 不做：不增加 reservation、每 key 并发上限、严格公平或本地 token bucket。

### TC-07：可观测性终态对称矩阵

- 可观察行为：已认证请求在 JSON 成功、SSE 成功、最终 HTTP error、timeout、EOF、失败 terminal、 incomplete terminal、error
  terminal、Bridge reject 和 cancellation 下恰好完成一次观测。
- 第一条测试：先选择 TC-01 或 TC-02 新增路径，同步断言 request/attempt/terminal counters 与脱敏字段。
- 扩展方式：每完成一种新 runtime replay，就在同一行为改动中补齐对应 observability 断言；不单独制造重复 transport mock。
- 不做：不引入 exporter、持久化、高基数 label 或业务正文采集。

### TC-08：拆分 ignored SDK 兼容测试

- 可观察行为：Python 与 Node SDK 的 Chat、Responses、stream、tool loop 和 429 失败可独立报告实际失败点。
- 第一条测试调整：先按 Python/Node 拆分两个 ignored Rust test；若单个脚本仍难定位，再按协议拆分脚本入口。
- 验证边界：显式执行 `cargo test --locked --test sdk_compatibility -- --ignored`，记录 SDK、运行时、平台和 安装来源。
- 不做：不把网络依赖加入默认 baseline，不长期固定外部 SDK 版本。

## 7. Canonical case 直接引用缺口快照

下列 19 个 case 当前未被 Rust 测试源码直接引用。它们是 process replay 的候选输入，不表示必须机械地为每个 case
创建独立测试函数；已有等价 synthetic contract 时，应优先增加生产 Router 的 fixture 连接，而不是复制断言。

### Transport（5）

- `chat_native.sse_framing`
- `responses_native.sse_framing`
- `responses_native.cancel.after_output`
- `responses_native.transport_error.after_output`
- `responses_native.transport_error.before_output`

### Fault（11）

- `chat_native.bad_gateway.non_stream`
- `chat_native.invalid_request.non_stream`
- `chat_native.permission_denied.non_stream`
- `chat_native.rate_limit.non_stream`
- `chat_native.unprocessable_entity.non_stream`
- `responses_native.authentication_error.non_stream`
- `responses_native.eof_before_terminal`
- `responses_native.gateway_timeout.non_stream`
- `responses_native.not_found.non_stream`
- `responses_native.rate_limit.http_date.non_stream`
- `responses_native.server_error.malformed_json.non_stream`

### Protocol（2）

- `chat_native.text.non_stream`
- `responses_native.text.non_stream`

### Regression（1）

- `responses_native.http_error.sse_content_type`

## 8. 执行与收敛规则

1. 从 TC-01 开始，或由明确缺陷替换为风险更高的单个行为；未进入 `current-focus.md` 的条目保持候选状态。
2. 先写或确认失败测试。若新增测试立即通过，只把它作为现有行为的回归证据，不虚构实现变更。
3. 先复用 canonical artifact 和现有 test support；只有第二个真实使用点出现后才抽取通用 helper。
4. 每个 runtime replay 至少断言上游 request、下游 status/headers/body 或 SSE semantics、terminal、attempt 数和
   commit/fallback 边界；需要时附加 observability 断言。
5. Rust 与 Python 重叠时保留一份 canonical wire，断言放在最接近责任的层；删除旧 helper 前先证明替代覆盖 相同 observable
   behavior 和 failure boundary。
6. 每完成一个条目，更新实施现状中的已证明事实和实际运行命令，再清空 `current-focus.md`。

## 9. 验证矩阵

### 只修改 Rust test/support

```powershell
cargo test --locked --test <focused-test-target> <test-name>
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

### 修改 `testdata/` 或 `tools/corpus/`

在 Rust baseline 之外追加：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

### 修改 SDK compatibility

默认 baseline 之外显式运行：

```powershell
cargo test --locked --test sdk_compatibility -- --ignored
```

真实 Provider、外部 SDK、负载、并发压力和长期运行结果必须与 deterministic test 分开报告。未执行的验收层不得 写成已验证。

## 10. 计划完成判据

本计划在满足以下条件后可以删除，其已确认事实转入 implementation-status：

- 19 个直接引用缺口逐项完成归属判定：要么由生产 Router replay 覆盖，要么记录已有 Rust owner 及不重复回放的 明确理由；
- transport 和 HTTP/SSE classification 的 P0 case 已经过生产 Router，且覆盖输出前后 commit boundary；
- 每个已编译 Provider 都有与其声明能力相符的 request、header、terminal 和 error profile 契约；
- credential/cooldown 的共享状态至少有一组无 sleep 的确定性并发覆盖；
- 新增 runtime replay 同步覆盖 observability 的唯一终态与脱敏不变量；
- SDK 测试失败可以按运行时和协议定位，同时仍保持 opt-in；
- 实施现状只记录实际运行过的验证，不保留本计划中的候选或未完成表述。

## 11. 非目标

- 不以行覆盖率、分支覆盖率或测试数量作为唯一完成标准；
- 不一次性把 45 个 case 都塞入一个难以定位失败的大测试；
- 不把 OpenBridge routing、retry、fallback、cooldown 或 credential policy 复制到 Python；
- 不新增 plugin system、动态测试控制面、独立发布 runner 或跨项目兼容层；
- 不使用 sleep 稳定并发测试；
- 不把真实 Provider 一次成功、SDK fixture 或 corpus 存在本身描述为整体兼容。
