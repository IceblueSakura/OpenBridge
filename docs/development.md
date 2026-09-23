# 开发指南

当前开发对象为 v2 Rust library 与独立离线验收。没有旧网关、auth、probe 或 MCP binary；旧 corpus/运行配置的使用方式见 [Git 归档](archive.md)，不是当前开发前置条件。

## 变更流程

1. 检查分支、Git status 与目标 diff；保留未提交工作，不 stage/commit/push。
2. 阅读 [v2 设计](architecture-v2/README.md)、受影响的源码/测试与固定协议资料。明确 owner、支持与拒绝边界。
3. 行为变更在 [current-focus](implementation-plans/current-focus.md)维护获准范围、可观察结果、失败用例和验证门槛，随后按失败证据实现。文档维护不制造运行时切片。
4. 测试在最低职责层保护独立语义；codec 的 decode 与 encode 分别使用独立预期，追加 IR 插入、替换、删除及对应失败边界。round trip 不能自证。
5. 检查最终 diff、文档与引用，报告实际检查和未验收范围。不得以删除旧代码或测试通过宣称生产功能已经迁移。

## Rust 检查

先运行受影响的 `semantic_v2_*` / `sse_contract` 测试，再执行：

```sh
cargo test --locked --offline
cargo clippy --locked --offline -- -D warnings
cargo fmt -- --check
git diff --check
```

`--offline` 需要预先可用的依赖缓存；不要为离线检查隐式调用 Provider。修改依赖后同步 `Cargo.lock`，再重跑 locked 检查，避免顺带升级无关依赖。

本机 Rust linker wrapper 缺失时，可仅对当前命令设置 `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang`；不要将主机 workaround 写成项目全局配置。

## 固定 OpenAI SDK loopback

```sh
uv run --offline --no-project --with openai==3.10.0 cargo test --locked --offline --test semantic_v2_responses_sdk_loopback -- --ignored --test-threads=1
```

这个显式 ignored gate 需要本地已有固定 SDK。测试专用 Router 只访问临时 literal loopback，以 synthetic Bearer 做两轮 JSON/SSE；不读取私有配置、不继承 Provider credential，不执行真实工具、环境代理或自动重试。不启动旧 OpenBridge 服务，也不证明真实 Provider、完整 Agent 或生产接线兼容。

`semantic_v2_body_lifecycle` 保护首帧、取消、背压与异常 body；`semantic_v2_responses_sse` 和 `sse_contract` 保护 framing 与终态。生命周期场景使用 channel/readiness 和有界 timeout，不用 sleep 隐藏竞争。子进程/listener/producer 需要失败路径清理。

## 文档与边界

Rust comments/docs 与 Python docstrings 使用简洁 English；重点解释协议、安全、资源和失败边界。Markdown 需检查相对链接、锚点、示例与当前 target 一致；结构性检查不证明行为改善。

不修改 `.env`、私人 `config/`、OAuth 文件，也不读取外部应用认证缓存。保留外部资料的版本、许可与 attribution；历史证据只能按当时边界解释。日志和诊断不能回显真实秘密或私有 payload。

完成时区分静态检查、Rust tests、固定 SDK/loopback、真实 Provider、负载和生产验证。当前正常基线没有任何真实 Provider、付费 API、部署或外部发布。
