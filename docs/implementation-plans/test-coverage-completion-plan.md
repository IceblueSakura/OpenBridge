# 测试补全计划

## 1. 状态与使用方式

**候选计划，不是当前实施焦点。** 本文件整理当前 checkout 的高价值测试缺口、补全顺序和验证边界，
不表示任一条目已经获准实施。开始具体工作前，必须只选择一个可观察行为，将对应内容写入
[当前开发焦点](current-focus.md)，完成测试与最小实现后再更新实施现状并清空当前焦点。

阶段 3 已完成首轮缺口确认：原先 19 个未直接引用的 canonical case 中，14 个已接入 production replay，剩余 5 个已记录
既有测试 owner 与不机械回放的理由。下列 P1 条目仍是后续候选，不属于本轮已实现事实。

本计划只补强现有产品行为的确定性证据，不改变产品范围、Provider 能力声明、Protocol Bridge 语义或
默认验收层。当前实现事实仍以[当前实现总览](../implementation-status/current-implementation.md)和
[协议测试语料与工具现状](../implementation-status/protocol-test-corpus.md)为准。

## 2. 当前测试基线

本快照于 2026-08-08 在阶段 3 高风险 canonical replay 补全后，从当前 checkout 的测试收集结果和 canonical catalog
得到：

| 测试资产               | 当前数量 | 主要责任                                                                  |
|------------------------|---------:|---------------------------------------------------------------------------|
| Rust 源码内单元测试    |       55 | 局部算法、状态、边界类型和安全不变量                                      |
| Rust 默认集成测试      |      228 | registry、routing、Provider、HTTP/SSE、retry/fallback、取消和 Bridge      |
| Python testkit 测试    |       36 | corpus、SSE parser、Mock Server/Client、observation verifier、生成与打包  |
| Canonical corpus       | 45 cases | Chat/Responses、Bridge、HTTP error、SSE terminal、transport 与取消 oracle |
| 默认 SSE wire variants |      306 | Python 所有的确定性 byte fragmentation 覆盖                               |

Rust 共收集到 283 个默认测试，不再保留 ignored 外部客户端测试。第一阶段删除了 7 个只锁定私有模块位置、指针身份、普通容器操作或
重复已存在强契约的测试，并把混合的 reasoning-output 测试收窄到唯一未重复的 MiMo Native 行为。静态 case-id 引用扫描显示，
45 个 canonical case 中有 40 个被 Rust 测试源码直接引用，5 个未被直接引用。这个数字只表示 fixture 与 Rust 测试的直接连接，
不等于 5 个行为完全没有 synthetic contract test，也不构成行覆盖率或分支覆盖率结论。

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
| 真实 Provider 差异                                                         | 独立 acceptance        | 不进入默认确定性测试，也不替代 fixture regression    |

不要为了提高数字在 Rust 中复制 306 个 Python fragmentation variants，也不要在 Python testkit 中实现 Route、 retry、fallback
或 credential rotation。

## 5. 优先级

| 优先级 | 补全方向                            | 当前缺口                                                                                  | 完成标志                                                                                    |
|--------|-------------------------------------|-------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| P0（已补） | Canonical case 经过生产 Router 回放 | 12 个 HTTP error 与 3 个 lifecycle case 已经过 production loopback                         | 高风险 transport/fault case 比较实际上游请求、下游响应、terminal、attempt 和 commit point   |
| P0（已补） | Streaming 失败与取消                | post-output abort、cancel 与 clean EOF 均有 production replay                              | 已证明三者在输出后不产生非法 retry/fallback 或伪造 terminal                                |
| P1     | Native response 公共身份            | formal response model 契约与当前 Native byte-transparent 实现/状态存在证据差异             | 独立焦点确定并验证 JSON/SSE 的 Public Model response projection，而不损坏未知合法 wire      |
| P1     | Provider wire profile               | Provider contract 数量较多，但每个 Provider 的 path、terminal、header 和 error 覆盖不对称 | 每个已编译 Provider 都有与声明能力匹配的表驱动契约                                          |
| P1     | credential/cooldown 并发            | 现有覆盖以顺序场景为主                                                                    | 并发请求下 cursor、cooldown、generation、attempt budget 和取消仍保持确定性不变量            |
| P1     | observability 终态矩阵              | 已有成功、失败、EOF、取消测试，但协议和多 attempt 场景不完全对称                          | 每个已支持终态只记录一次，计数正确，诊断不含正文和 credential                               |

## 6. 候选短周期行为

以下条目是候选工作，不应在一次改动中全部实施。每次只将一个条目转入 `current-focus.md`。

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
- 第一条测试：随下一条新增 runtime replay 同步断言 request/attempt/terminal counters 与脱敏字段。
- 扩展方式：每完成一种新 runtime replay，就在同一行为改动中补齐对应 observability 断言；不单独制造重复 transport mock。
- 不做：不引入 exporter、持久化、高基数 label 或业务正文采集。

### TC-08：Native response 使用 Public Model 身份

- 可观察行为：Native Chat/Responses 的 JSON 与 SSE response 中，协议定义的 `model` 字段使用下游 Public Model；其他未知合法
  字段、framing、event 顺序和 payload 仍保持透明。
- 已确认差异：formal Public Model 契约把 Public Model 定义为下游 response identity，canonical native
  `expected-client-*` 也执行该投影；当前实现现状则明确记录 Native response bytes 透明，production replay 实际匹配
  `upstream-stream.sse` 而不是 `expected-client-stream.sse`。
- 第一条测试：单独以 `responses_native.text.non_stream` 建立失败的 production replay，再决定 bounded JSON/SSE projection；
  streaming transport/cancellation 测试只断言生命周期，不顺带固定该行为。
- 不做：不通用解析或重写未知字段，不把 Bridge renderer 用于 Native，不在未进入独立 `current-focus.md` 前修改生产行为。

## 7. Canonical case 直接引用缺口归因

阶段 3 开始时有 19 个 case 未被 Rust 源码直接引用。production replay 现已补入 10 个 HTTP fault case，以及
`responses_native.http_error.sse_content_type`、`responses_native.transport_error.after_output`、
`responses_native.cancel.after_output` 和 `responses_native.eof_before_terminal`，因此直接引用为 40/45。

剩余 5 个 case 已有下列行为 owner 或证据冲突；在对应产品行为单独获准前，不用重复测试或错误命名的 replay 消除数字缺口：

| Case | 当前 owner | 暂不直接回放的原因 |
|------|------------|--------------------|
| `chat_native.text.non_stream` | `forwarding_contract/native.rs` 的 Chat/Responses Native forwarding contract | canonical expected response 会把 upstream model 投影为 Public Model；当前 Native 实现与状态记录为 byte-transparent，需先完成 TC-08 行为决策 |
| `responses_native.text.non_stream` | 同一 Native forwarding contract | 同上；它是 TC-08 建立失败 production replay 的首选 case |
| `chat_native.sse_framing` | `sse_contract.rs` 的 incremental framing contract 与 Native forwarding contract | decoder/framing 已有 synthetic owner，但 canonical expected stream 同样包含未实现的 Public Model response projection |
| `responses_native.sse_framing` | 同一 SSE 与 Native forwarding owner | 同上；不能在 lifecycle test 中顺带固定 response identity |
| `responses_native.transport_error.before_output` | `forwarding_contract/resilience.rs` 的 before-output retry/fallback contracts 与 Provider error classifier | case 名称/features 写 transport error，但 transport oracle 实际声明 HTTP 503 `error_response`；直接当 socket transport replay 会混淆两种失败类别 |

## 8. 执行与收敛规则

1. 先以 TC-08 解决 response identity 证据差异，再继续 TC-04；也可由明确缺陷替换为风险更高的单个行为。未进入
   `current-focus.md` 的条目保持候选状态。
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

真实 Provider、外部 SDK、负载、并发压力和长期运行结果必须与 deterministic test 分开报告。未执行的验收层不得 写成已验证。

## 10. 计划完成判据

本计划在满足以下条件后可以删除，其已确认事实转入 implementation-status。前两项已由阶段 3 满足，其余仍是后续候选：

- 已满足：19 个初始直接引用缺口逐项完成归属判定；14 个接入 production replay，5 个记录既有 Rust owner 与不重复回放理由；
- 已满足：高风险 HTTP/SSE classification、post-output abort、cancel 与 EOF case 已经过 production Router；
- 每个已编译 Provider 都有与其声明能力相符的 request、header、terminal 和 error profile 契约；
- credential/cooldown 的共享状态至少有一组无 sleep 的确定性并发覆盖；
- 新增 runtime replay 同步覆盖 observability 的唯一终态与脱敏不变量；
- 实施现状只记录实际运行过的验证，不保留本计划中的候选或未完成表述。

## 11. 非目标

- 不以行覆盖率、分支覆盖率或测试数量作为唯一完成标准；
- 不一次性把 45 个 case 都塞入一个难以定位失败的大测试；
- 不把 OpenBridge routing、retry、fallback、cooldown 或 credential policy 复制到 Python；
- 不新增 plugin system、动态测试控制面、独立发布 runner 或跨项目兼容层；
- 不使用 sleep 稳定并发测试；
- 不把真实 Provider 一次成功、SDK fixture 或 corpus 存在本身描述为整体兼容。
