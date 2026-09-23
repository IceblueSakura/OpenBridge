# OpenBridge v2

OpenBridge 正在重建为以任务语义 IR 为权威的 OpenAI-compatible gateway。**当前工作区是 Rust 库与离线验收，不是可运行网关。**

旧 service、auth、probe、Provider/registry、MCP、观测及 gateway-tools 原型已整体退役；其源码、测试、配置模板、运行文档和 corpus 在 [Git 归档](docs/archive.md)中查阅。它们不代表 v2 已实现能力。未提供监听入口，不读取私有配置或凭据。

## 当前范围

```text
Chat / Responses wire
 → protocol decode
 → Generation IR / trusted transform
 → requirements / lowering
 → protocol encode
 → JSON / SSE
```

- `src/semantic/`：Generation typed request、response、event、验证和 requirements。
- `src/protocol/`：Chat/Responses codec、表示元数据和 Responses HTTP/SSE 边界。
- `src/lowering/`：针对固定表示契约的可表示性检查。
- `src/transport/sse.rs`：有界纯 SSE framing。
- `tests/semantic_v2_*`、`tests/sse_contract.rs`：独立语义及失败边界测试；SDK/HTTP 只使用 synthetic loopback。

Responses 纯文本验收仍在推进，见 [当前焦点](docs/implementation-plans/current-focus.md)和[准入说明](docs/architecture-v2/responses-text-profile.md)。媒体、其他任务、生产执行与 Provider 接入尚未完成；删除旧路线不等于这些功能已迁移。

## 验证

需要 Rust 2024 工具链。已有依赖缓存时运行：

```sh
cargo test --locked --offline
cargo clippy --locked --offline -- -D warnings
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
