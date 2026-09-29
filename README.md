# OpenBridge v2

OpenBridge 的最终目标是**支持多模态、兼容 Chat Completions / Responses、Agent 友好且缓存亲和性强的 IR 化网关**。现阶段以仅文本 Generation 验证整体流程，之后再扩展 Provider 与多模态实现；不是长期只做文本 codec。产品判据见 [v2 目标](docs/architecture-v2/README.md#产品目标与阶段判据)。

Generation IR **以 OpenAI Responses 标准语义为主干，结合有明确归属和生命周期的扩展字段**，而非多协议最小公分母。设计依据见[主题化调研与上游同步](docs/references/README.md)。**当前工作区是 Rust 库与离线验收，不是完整标准实现或可运行网关。**

旧 service、auth、probe、Provider/registry、MCP、观测及 gateway-tools 原型已整体退役；其源码、测试、配置模板、运行文档和 corpus 在 [Git 归档](docs/archive.md)中查阅。它们不代表 v2 已实现能力。库未提供监听入口，不读取私有配置或凭据；受控 `examples/live_probe.rs` 有独立运行授权与凭据边界，不属于默认验证。

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
- `src/provider/`、`src/topology/`、`src/execution/`：可信绑定、固定候选、增量 intake/delivery 与显式 I/O commit；网络仍由 caller 提供。
- `tests/semantic.rs`、`tests/transport.rs`、`tests/sdk_loopback.rs`：语义、transport 与固定 SDK 三个验收入口；SDK/HTTP 只使用 synthetic loopback。

纯文本 Generation 已具备请求、响应、事件、IR 变换与双协议编码的离线主链；Responses 为语义主干，[Chat 单候选 JSON/SSE profile](docs/architecture-v2/chat-text-profile.md)验证同一 IR 的第二协议投影。**仍有已准入边界的正确性缺口，不能称完整标准实现。** [完成度与缺口](docs/architecture-v2/migration.md)区分现有能力、错误接受、尚未映射的文本字段和明确非目标；[下一步建议](docs/implementation-plans/next-goal.md)在已有 library execution 上推进最小 ingress/交付接线，按场景补必要 Chat 投影。媒体、其他任务和生产 Provider 接入未完成；已选定行为切片的范围由[当前焦点](docs/implementation-plans/current-focus.md)维护。

## 验证

Rust/Cargo 由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定。已有工具链与依赖缓存时运行：

```sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
git diff --check
```

固定 OpenAI SDK gate 单独显式运行，命令与安全边界见[开发指南](docs/development.md)。`cargo run` 不再提供旧服务入口。

## 文档

- [文档索引](docs/README.md)
- [当前结构](docs/architecture.md)
- [v2 设计](docs/architecture-v2/README.md)
- [迁移与未完成边界](docs/architecture-v2/migration.md)
- [下一步目标](docs/implementation-plans/next-goal.md)
- [外部协议参考](docs/references/README.md)

原创代码和文档采用 [MIT License](LICENSE)。外部资料保留各自来源与必要 attribution。
