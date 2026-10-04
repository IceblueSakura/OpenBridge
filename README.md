# OpenBridge

OpenBridge 建立**可由 Gateway 与未来自研 Agent 复用的模型交互 Semantic Model / IR**，以尽量低的语义损失连接不同 Provider，并向下游提供稳定的标准 API。项目尚未上线；优先稳定概念、所有权与不变量，不冻结当前 Rust 类型或照搬协议 DTO。

当前主线是 Agent-first 的 Text/Image/File 交互，优先完善规范 Responses；Chat Completions 仅作允许声明损失的兼容路径。Embedding 与独立标准媒体 operation 后续选片，具体端点范围另行讨论。音频 Realtime 明确要实现，但推迟设计与实施以降低每阶段关注度。有效合同归[语义架构](docs/architecture-v2/README.md)，具体步骤归[后续计划](docs/implementation-plans/next-goal.md)，不代表当前能力已经扩大。

## 当前范围

当前工作区包含 Rust 语义库、统一文件凭据管理器与最小认证 loopback Generation 网关，不是完整标准实现或生产就绪服务。

- HTTP 入口为 Chat Completions / Responses，提供受限的无状态文本输出与选定 URL/inline 图片输入。工具图片结果有独立准入，不能由 user 图片支持推定；协议、模型与实例启用分别核查。
- 同协议与跨协议都走 adapter → IR → validation/transform → requirements/lowering → adapter → JSON/SSE；不可表示的语义明确拒绝，不承诺任意无损转换。
- 凭据只从操作者指定的自有文件加载；显式池策略允许受预算约束的提交前 fallback，不提供普通请求内登录、自动 refresh、负载均衡或会话管理。
- 缓存亲和利用 Provider 原生功能和声明的 carrier，不实现网关回答缓存；前缀稳定不证明命中或收益。

当前模块接线见[架构](docs/architecture.md)，尚未闭合的语义、表示、执行与验收范围见[Generation 缺口](docs/implementation-status/generation.md)。推进方向由[next-goal](docs/implementation-plans/next-goal.md)维护，获准行为切片由[current-focus](docs/implementation-plans/current-focus.md)维护；设计或计划不授予操作权限。

## 验证

语义库构造不读取私有配置。`openbridge` binary 通过显式入口配置与凭据目录启动；命令本身不发生成请求：

```sh
cargo run --locked --offline --bin openbridge -- --credentials-dir /path/to/private-store
```

先按[HTTP 指南](docs/http-gateway.md)准备入口配置，账户与池操作见[凭据指南](docs/credentials.md)。真实登录、推理或付费测试需独立授权，使用[受控 probe](docs/probes.md)，不属于默认检查。

Rust/Cargo 由[`rust-toolchain.toml`](rust-toolchain.toml)固定；已有工具链与依赖缓存时：

```sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
git diff --check
```

Python/TS 离线防线与单独显式运行的固定 OpenAI SDK loopback gate 见[开发指南](docs/development.md)。检查结果只在当次交付中报告；synthetic 执行不证明真实 Provider、一般 SDK/Agent、网络、负载或生产兼容性。

## 文档

实现事实优先由源码、邻近注释和独立测试维护。当前 Provider/model 与实例准入按 [AGENTS 查询方法](AGENTS.md#current-provider-model-and-compatibility-information)现场核对，不在 Markdown 或记忆中维护库存及测试结果。

[文档索引](docs/README.md)区分设计合同、实现缺口、操作指南和[固定来源](docs/references/README.md)；旧源码只按[归档定位](docs/archive.md)查 Git，不是当前兼容要求。

原创代码和文档采用 [MIT License](LICENSE)。外部资料保留各自来源与必要 attribution。
