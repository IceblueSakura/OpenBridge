# 开发指南

当前开发对象为 v2 Rust library 与独立离线验收。没有旧网关、auth、probe 或 MCP binary；旧 corpus/运行配置的使用方式见 [Git 归档](archive.md)，不是当前开发前置条件。

## 变更流程

1. 检查分支、Git status 与目标 diff；保留未提交工作，不 stage/commit/push。
2. 阅读 [v2 设计](architecture-v2/README.md)、受影响的源码/测试与固定协议资料。明确 owner、支持与拒绝边界。
3. 行为变更在 [current-focus](implementation-plans/current-focus.md)维护获准范围、可观察结果、失败用例和验证门槛，随后按失败证据实现。文档维护不制造运行时切片。
4. 测试在最低职责层保护独立语义；codec 的 decode 与 encode 分别使用独立预期，追加 IR 插入、替换、删除及对应失败边界。round trip 不能自证。
5. 检查最终 diff、文档与引用，报告实际检查和未验收范围。不得以删除旧代码或测试通过宣称生产功能已经迁移。

## Rust 检查

Rust/Cargo 由根 `rust-toolchain.toml` 固定；rustfmt/clippy 随该工具链安装。不要保留覆盖这个文件的旧目录级 rustup override，也不要为项目更新全局默认工具链。

集成测试收敛为三个入口，按职责筛选，不再按迁移批次增加 binary：

| Target | 模块与边界 |
|---|---|
| `semantic` | `tests/semantic/`：instructions、tools、reasoning、schema、text profile/events、function events、response；纯语义与 codec/lowering |
| `transport` | `tests/transport/`：framing、Responses/Chat SSE、Chat envelope、body lifecycle；基础 framer 和真实 body I/O 各自验证 |
| `sdk_loopback` | 显式 ignored 的固定 Python SDK 的 Responses/Chat 两轮 JSON/SSE gates，不进入默认外部依赖检查 |

`tests/support/` 只共享 synthetic builders 和独立 wire 预期，不从被测 encoder 生成 oracle。相同字段的 decode、独立 encode、变换、失败和 I/O 可能保护不同边界，不按测试数量裁剪；删除重复 smoke/自比较检查前，确认剩余独立预期覆盖其有效断言。

先运行受影响模块，例如：

```sh
cargo test --locked --offline --test semantic reasoning::
cargo test --locked --offline --test transport
```

再执行完整基线（clippy 的 `--all-targets` 也检查测试与 helper）：

```sh
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
cargo fmt -- --check
git diff --check
```

`--offline` 需要预先可用的依赖缓存；不要为离线检查隐式调用 Provider。修改依赖后同步 `Cargo.lock`，再重跑 locked 检查，避免顺带升级无关依赖。

## 固定 OpenAI SDK loopback

Python 版本由 `tests/sdk/.python-version` 固定；OpenAI SDK 与测试环境 pip 在 `tests/sdk/pyproject.toml` 声明，全部传递依赖和下载 hash 由 `tests/sdk/uv.lock` 固定。环境只安装到被忽略的 `tests/sdk/.venv/`，不向系统 Python 安装 pip/package。不要直接 `pip install -U` 让环境偏离锁文件。

首次准备需要依赖下载；已有缓存可为 sync 加 `--offline`：

```sh
uv sync --project tests/sdk --locked
uv run --project tests/sdk --locked --offline python -m pip check
uv run --project tests/sdk --locked --offline cargo test --locked --offline --test sdk_loopback -- --ignored --test-threads=1
```

该 target 显式运行两个 gates：Responses 覆盖 function/custom/reasoning 历史；Chat 覆盖单候选 function 两轮，通过 SDK create 和 typed chunks 消费。两者请求使用各自完整 envelope bytes 入口，synthetic 响应经静态 bytes / SSE decoder；SDK 使用严格响应验证，最终正文来自修改后的 IR。它们不是全部 SDK create/parse/replay 分支的验收。测试专用 Router 只访问临时 literal loopback，使用 synthetic Bearer；不读取私有配置、不继承 Provider credential，不执行真实工具、环境代理或自动重试。不启动旧 OpenBridge 服务，也不证明真实 Provider、完整 Agent 或生产接线兼容。

`transport::body_lifecycle` 保护首帧、取消、背压与异常 body；`transport::responses_sse`、`transport::chat` 和 `transport::framing` 分别保护协议 adapter 与共用 framer 的终态。生命周期场景使用 channel/readiness 和有界 timeout，不用 sleep 隐藏竞争。子进程/listener/producer 需要失败路径清理。

## 文档与边界

Rust comments/docs 与 Python docstrings 使用简洁 English；重点解释协议、安全、资源和失败边界。Markdown 需检查相对链接、锚点、示例与当前 target 一致；结构性检查不证明行为改善。

不修改 `.env`、私人 `config/`、OAuth 文件，也不读取外部应用认证缓存。保留外部资料的版本、许可与 attribution；历史证据只能按当时边界解释。日志和诊断不能回显真实秘密或私有 payload。

完成时区分静态检查、Rust tests、固定 SDK/loopback、真实 Provider、负载和生产验证。当前正常基线没有任何真实 Provider、付费 API、部署或外部发布。
