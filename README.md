# OpenBridge

OpenBridge 的最终目标是**以一套 Agent-first、协议中立的 IR 兼容多种 API 协议，支持多模态与 Provider 原生缓存的网关**。项目尚未上线，处于设计探索阶段；稳定目标是语义、所有权和交互不变量，不是现有类型。当前实现从 Chat Completions / Responses 的文本 Generation 与选定图片输入起步，不是长期只做文本 codec。产品判据见 [v2 目标](docs/architecture-v2/README.md#产品目标与阶段判据)。

Generation IR **以有序交互、行动与结果、控制转移和续轮依赖为设计主线**。OpenAI Responses、Google Gemini、Anthropic Messages 是共同参考，不是表达力上限；既不取最小公分母，也不机械合并各家字段。设计依据见[决策与语义设计](docs/architecture-v2/README.md)，外部出处见[来源入口](docs/references/README.md)。**当前工作区包含 Rust 语义库与最小 loopback Generation 网关，不是完整标准实现或生产就绪服务。**

语义库构造不读取私有配置。`openbridge` binary 通过显式环境变量启动认证的 loopback HTTP 入口；独立 auth CLI 只访问操作者指定的自有 store。启动方式见 [HTTP 网关指南](docs/http-gateway.md)，账户操作与 access 绑定见[凭据指南](docs/credentials.md)。真实测试须通过明确授权的 [probe](docs/probes.md)，不属于默认验证。

下一步 IR 设计优先定稿**交互与续轮、分组及 replay 依赖**，再展开资源/工具结果、cache/usage 与上下文演进。认证与推理是独立职责；账户绑定不授权自动刷新、切换或重试。未完成方向由 [next-goal](docs/implementation-plans/next-goal.md) 维护。

## 当前范围

```text
Chat / Responses wire
 → Client / Provider adapter decode
 → Generation IR + context / delivery / trusted transform
 → requirements / target lowering
 → Provider / Client adapter encode
 → JSON / SSE
```

- `src/semantic/`：Generation typed request、response、event、标准 context/delivery、验证和 requirements。
- `src/adapter/`：显式 client/Provider profiles、统一 request 表示与目标投影（含声明的 Provider cache key/session carrier）；允许有合同的字段归一化，不另建厂商 IR。
- `src/protocol/`：Chat/Responses codec、表示元数据及各自的完整 JSON/SSE 边界。
- `src/lowering/`：针对固定表示契约的可表示性检查。
- `src/transport/sse.rs`：有界纯 SSE framing。
- `src/provider/`、`src/topology/`、`src/execution/`：可信绑定、固定候选、增量 intake/delivery 与显式 I/O commit。
- `src/gateway/`、`src/transport/http.rs`、`src/bin/openbridge.rs`：认证入口、可信预算变换、HTTP I/O 与实际 body handoff；无同候选重试，只有显式受信 Route 策略允许提交前有界 fallback。
- `tests/semantic.rs`、`tests/transport.rs`、`tests/gateway.rs`、`tests/sdk_loopback.rs`：语义、transport、真实 Router/binary 与固定 SDK 验收；HTTP 测试只使用 synthetic loopback。

**当前范围是受限的无状态文本 Generation，以及选定的 user / Responses 工具结果 URL/inline 图片输入→文本输出；不是完整多模态标准实现或生产就绪服务。** 工具图片的语义准入独立于 user 图片，codec 支持不代表 catalog 或实际实例已启用；具体边界见[工具图片结果合同](docs/architecture-v2/responses-text-profile.md#tool-image-results)。 图片语义与明确拒绝范围见[图片输入合同](docs/architecture-v2/responses-text-profile.md#user-image-input)，实际模型/Endpoint 准入仍须现场查询。当前类型和 codec 仍以 Responses/单候选 Chat 切片为实现基础；新的协议中立设计不代表额外协议已接入，跨协议不可表示时仍拒绝。尚未闭合的语义、接线与验收范围见[实施边界与缺口](docs/implementation-status/generation.md)；推进方向见[下一步目标](docs/implementation-plans/next-goal.md)，获准行为切片由[当前焦点](docs/implementation-plans/current-focus.md)维护。

缓存亲和只利用 Provider 原生自动缓存和明确字段，维护稳定前缀；不实现网关负载均衡、回答缓存或会话管理。稳定合同与扩展 owner 见 [ADR 0011](docs/architecture-v2/decisions/0011-stable-admission-provider-cache.md)。

## 验证

Rust/Cargo 由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定。已有工具链与依赖缓存时运行：

```sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
git diff --check
```

Python/probe、TS 类型检查与 Node 单测、固定 OpenAI SDK gate 的命令和安全边界见[开发指南](docs/development.md)；SDK gate 单独显式运行。`cargo run --bin openbridge` 启动最小网关，需先按网关指南提供显式启动凭据。

## 文档

优先以代码、邻近注释和独立测试维护实现事实。当前 Provider/模型与协议准入不在 Markdown 列表中维护；按 [AGENTS.md 的查询方法](AGENTS.md#current-provider-model-and-compatibility-information)现场核对代码与启动绑定，实际实例启用和上游可用性另行验证。

- [文档索引](docs/README.md)
- [HTTP 网关启动与接口](docs/http-gateway.md)
- [Grok / Codex 共用凭据管理](docs/credentials.md)
- [受控 Probe 计划、预算与诊断](docs/probes.md)
- [当前结构](docs/architecture.md)
- [v2 设计](docs/architecture-v2/README.md)
- [Generation 当前能力与边界](docs/implementation-status/generation.md)
- [下一步目标](docs/implementation-plans/next-goal.md)
- [外部协议参考](docs/references/README.md)

原创代码和文档采用 [MIT License](LICENSE)。外部资料保留各自来源与必要 attribution。
