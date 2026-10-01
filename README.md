# OpenBridge

OpenBridge 的最终目标是**支持多模态、兼容 Chat Completions / Responses、Agent 友好且缓存亲和性强的 IR 化网关**。现阶段在文本 Generation 主链上扩展选定的图片输入→文本输出 slice，再逐步推进其他多模态；不是长期只做文本 codec。产品判据见 [v2 目标](docs/architecture-v2/README.md#产品目标与阶段判据)。

当前 `main` 以原 `semantic-v2` 实现为基线，独立推进产品目标；不以迁移、追平或恢复旧版本为目标。旧版只作为 Git refs 与[历史参考](docs/archive.md)保留，主线定位不等于完整标准或生产就绪。

Generation IR **以 OpenAI Responses 标准语义为主干，结合有明确归属和生命周期的扩展字段**，而非多协议最小公分母。设计依据见[主题化调研与上游同步](docs/references/README.md)。**当前工作区包含 Rust 语义库与最小 loopback Generation 网关，不是完整标准实现或生产就绪服务。**

旧 service、auth、probe、Provider/registry、MCP、观测及 gateway-tools 原型已整体退役；其源码、测试、配置模板、运行文档和 corpus 在 [Git 归档](docs/archive.md)中查阅。它们不代表 v2 已实现能力。库构造不读取私有配置；`openbridge` binary 通过显式环境变量启动认证的 loopback HTTP 入口，不读取旧配置。启动方式、限制与接口见 [HTTP 网关指南](docs/http-gateway.md)。受控 `examples/live_probe.rs` 仍有独立运行授权与凭据边界，不属于默认验证。

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
- `src/adapter/`：显式 client/Provider profiles、统一 request 表示与目标投影；允许有合同的字段归一化，不另建厂商 IR。
- `src/protocol/`：Chat/Responses codec、表示元数据及各自的完整 JSON/SSE 边界。
- `src/lowering/`：针对固定表示契约的可表示性检查。
- `src/transport/sse.rs`：有界纯 SSE framing。
- `src/provider/`、`src/topology/`、`src/execution/`：可信绑定、固定候选、增量 intake/delivery 与显式 I/O commit。
- `src/gateway/`、`src/transport/http.rs`、`src/bin/openbridge.rs`：认证入口、可信预算变换、HTTP I/O 与实际 body handoff；无同候选重试，只有显式受信 Route 策略允许提交前有界 fallback。
- `tests/semantic.rs`、`tests/transport.rs`、`tests/gateway.rs`、`tests/sdk_loopback.rs`：语义、transport、真实 Router/binary 与固定 SDK 验收；HTTP 测试只使用 synthetic loopback。

**受限的无状态 text Generation 主链已闭合，并准入选定的 user URL/inline 图片输入→文本输出 slice；不是完整多模态标准实现或生产就绪服务。** 图片语义与明确拒绝范围见[图片输入合同](docs/architecture-v2/responses-text-profile.md#user-image-input)，实际模型/Endpoint 准入仍须现场查询。 Responses 为语义主干，单候选 Chat 是同一 IR 的第二协议投影，跨协议不可表示时拒绝。语义、codec、扩展接线、SDK/Agent、缓存与执行的分层判断统一见[当前能力与边界](docs/implementation-status/generation.md)；推进方向见[下一步目标](docs/implementation-plans/next-goal.md)，获准行为切片由[当前焦点](docs/implementation-plans/current-focus.md)维护。

## 验证

Rust/Cargo 由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定。已有工具链与依赖缓存时运行：

```sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
git diff --check
```

固定 OpenAI SDK gate 单独显式运行，命令与安全边界见[开发指南](docs/development.md)。`cargo run --bin openbridge` 提供新的最小入口，不恢复旧服务；需先按网关指南提供显式启动凭据。

## 文档

优先以代码、邻近注释和独立测试维护实现事实。当前 Provider/模型与协议准入不在 Markdown 列表中维护；按 [AGENTS.md 的查询方法](AGENTS.md#current-provider-model-and-compatibility-information)现场核对代码与启动绑定，实际实例启用和上游可用性另行验证。

- [文档索引](docs/README.md)
- [HTTP 网关启动与接口](docs/http-gateway.md)
- [受控 Probe 计划、预算与诊断](docs/probes.md)
- [当前结构](docs/architecture.md)
- [v2 设计](docs/architecture-v2/README.md)
- [Generation 当前能力与边界](docs/implementation-status/generation.md)
- [下一步目标](docs/implementation-plans/next-goal.md)
- [外部协议参考](docs/references/README.md)

原创代码和文档采用 [MIT License](LICENSE)。外部资料保留各自来源与必要 attribution。
