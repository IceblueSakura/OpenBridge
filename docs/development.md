# 开发指南

当前开发对象为 v2 Rust library（semantic、adapter、topology、execution）与独立离线验收。最小 `openbridge` binary 的启动见 [HTTP 网关指南](http-gateway.md)；没有旧网关或 MCP binary；受控 live probe example 不是默认验证入口，旧 corpus/运行配置的使用方式见 [Git 归档](archive.md)，不是当前开发前置条件。

## 变更流程

1. 检查分支、Git status 与目标 diff；保留未提交工作，不 stage/commit/push。
2. 阅读 [v2 设计](architecture-v2/README.md)、受影响的源码/测试与固定协议资料。明确 owner、支持与拒绝边界。
3. 行为变更在 [current-focus](implementation-plans/current-focus.md)维护获准范围、可观察结果、失败用例和验证门槛，随后按失败证据实现。文档维护不制造运行时切片。
4. 测试在最低职责层保护独立语义；codec 的 decode 与 encode 分别使用独立预期，追加 IR 插入、替换、删除及对应失败边界。round trip 不能自证。
5. 检查最终 diff、文档与引用，报告实际检查和未验收范围。不得以删除旧代码或测试通过宣称生产功能已经迁移。

## Rust 检查

Rust/Cargo 由根 `rust-toolchain.toml` 固定；rustfmt/clippy 随该工具链安装。不要保留覆盖这个文件的旧目录级 rustup override，也不要为项目更新全局默认工具链。

集成测试按以下入口分工，按职责筛选，不再按迁移批次增加 binary：

| Target | 模块与边界 |
|---|---|
| `semantic` | `tests/semantic/`：instructions、phase、tools、reasoning、schema、parsed replay、extensions、text profile/events、function events、response、chat wire、adapter profile 隔离、usage 缺省规则与来源/依赖保真；纯语义与 codec/lowering |
| `transport` | `tests/transport/`：framing、Responses/Chat SSE、Chat envelope、body lifecycle；基础 framer、增量 Attempt/ResponseDelivery、实际 I/O commit 边界和 synthetic body I/O 各自验证 |
| `gateway` | 一个真实 Router→synthetic HTTP Provider smoke；另一个隔离环境 binary bootstrap gate，使用 synthetic keys 与拒绝出站的 loopback 代理，不调用真实 Provider |
| `sdk_loopback` | 显式 ignored 的固定 Python SDK codec fixture gates，以及真实 Gateway Router→synthetic Provider 的双协议 JSON/SSE 续轮 gate，不进入默认外部依赖检查 |

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

该 target 显式运行三个 gates。原有两个 codec gates：Responses 覆盖 function/custom/reasoning 历史与派生视图回放（三轮）；Chat 覆盖单候选 function 三轮，通过 SDK `parse()`/stream helper 与 create/typed chunks 消费后把携带派生 view 的真实 dump 回放进后续请求。两者请求使用各自完整 envelope bytes 入口，synthetic 响应经静态 bytes / SSE decoder；SDK 使用严格响应验证，最终正文来自修改后的 IR，回放请求的 raw body 权威性在服务端断言。它们只用 `parse()` 产生真实派生 dump 以验收回放准入，不构成 parse/parsed 派生、模型输出 adherence 或全部 replay 分支的验收。测试专用 Router 只访问临时 literal loopback，使用 synthetic Bearer；不读取私有配置、不继承 Provider credential，不执行真实工具、环境代理或自动重试。不启动旧 OpenBridge 服务，也不证明真实 Provider、完整 Agent 或生产就绪。

新增 `gateway_sdk::sdk_uses_gateway_for_both_protocols_and_deliveries` 使用实际 `Gateway::serve` 与独立 synthetic HTTP Provider。固定 SDK 的 Chat/Responses JSON/SSE 请求经过认证、模型绑定、缺省 token 预算写入 IR、request lowering、真实 HTTP、response/event adapter 与 HTTP body，再回放实际工具/derived views/encrypted reasoning。上游 oracle 独立检查 model 重绑定、默认预算、call ID、原始参数和工具结果；不由网关 handler 直接构造客户端回答。所有凭据为 synthetic，SDK/transport 禁止环境代理继承与重试，进程有 deadline 与取消清理。该 gate 不代替真实 Provider 经新 binary 的验收。

`transport::body_lifecycle` 保护首帧、取消、背压与异常 body；`transport::responses_sse`、`transport::chat` 和 `transport::framing` 分别保护协议 adapter 与共用 framer 的终态。生命周期场景使用 channel/readiness 和有界 timeout，不用 sleep 隐藏竞争。子进程/listener/producer 需要失败路径清理。

## 受控真实 Provider gates

真实调用需当次明确授权目标、矩阵、token/请求数和报告边界；以下入口不属于默认 tests，也不因环境中存在密钥就获准运行。

- `examples/live_probe.rs`：显式 `OPENBRIDGE_PROBE=1`；OpenRouter 和额外五家 API-key Provider 必须显式选择 `OPENBRIDGE_PROBE_MODEL`。只遍历已声明的 native protocol；新 Provider 不会自动加入原默认付费矩阵。`OPENBRIDGE_PROBE_PROTOCOL`、`OPENBRIDGE_PROBE_CASE`、`OPENBRIDGE_PROBE_DELIVERY` 只缩小固定矩阵，`OPENBRIDGE_PROBE_MAX_TOKENS` 限于 1–2048。真实工具输出按完整已交付历史续轮，报告同时检查终态和场景 oracle，失败退出非零。
- `OPENBRIDGE_PROBE_REPLAY_DIR=<existing probe directory> cargo run --locked --offline --example live_probe`：离线回放该目录的固定 GPT-6 Luna 首轮 captures，不读取凭据、不联网；缺失/失败样本不会被计为通过。原始 captures 不进入独立 fixtures。
- `examples/live_gateway_probe.py`：先 `cargo build --locked --offline --bin openbridge`，再经固定环境显式运行 `OPENBRIDGE_GATEWAY_PROBE=1 uv run --project tests/sdk --locked --offline python examples/live_gateway_probe.py`。默认最多 12 次 synthetic GPT-6 Luna 请求；追加 `OPENBRIDGE_GATEWAY_REASONING=1` 改为 8 次加密状态获取/续轮，首轮必须实际有密文，第二轮在 SDK `send` 前以内存比较验证序列化后的 issuer ID/密文未丢失或变化。两种模式均每次最多 2048 输出 tokens，SDK → 临时 loopback binary → Provider；SDK 不继承代理，网关仅使用显式可信 egress。使用固定 SDK 导出的 `DefaultHttpxClient`，不新增未锁定的 Python HTTP 依赖。

- `examples/live_provider_matrix.py`：`OPENBRIDGE_PROVIDER_MATRIX=1 uv run --project tests/sdk --locked --offline python examples/live_provider_matrix.py`。需先构建当前 binary；最多 66 次生成，每次 2048 输出 tokens，覆盖八个固定 API-key 绑定的已声明协议 × JSON/SSE × 文本/工具及续轮。`OPENBRIDGE_MATRIX_PROVIDERS`（逗号分隔）、`OPENBRIDGE_MATRIX_PROTOCOL`、`OPENBRIDGE_MATRIX_DELIVERY`、`OPENBRIDGE_MATRIX_CASE` 只缩小矩阵。任何 Provider 首个失败后停止其余场景；无实际调用或任一失败都退出非零。DeepSeek 两个协议与 NVIDIA Chat 使用 auto 工具选择并独立断言实际调用和结果，不把强制工具选择的不兼容掩盖成语义转换。
- `cargo run --locked --offline --example replay_chat -- longcat <authorized-synthetic-capture.sse>`：对明确给出的 2 MiB 内 capture 做纯离线 intake 和下游 projection，不读取凭据、不联网，不把 captures 当独立 fixture。
- `uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p 'test_*.py'`：既有 replay checker 和新增矩阵请求数/目标/输出预算的离线防线，无真实调用。

这些 live 入口只在进程内读取工作区 API-key 凭据，均无自动重试；不会更改私有文件或加载 Codex/OAuth 引用。库级 probe 将最多 2 MiB 的有界诊断写到唯一、被忽略的 `testdata/runtime/` 子目录；SDK/binary gate 只保存脱敏结果，不保存 body/headers/token。后者通过启动就绪信号获取临时端口，结束/失败均回收 binary。每次重新执行都是新的一批付费调用，不是“免费重跑测试”。已有执行结果见 [GPT-6 Luna evidence](implementation-status/evidence/2026-09-29-openrouter-luna-acceptance.md)与 [reasoning 专项](implementation-status/evidence/2026-09-29-reasoning-continuation-acceptance.md)。probe 序列化守卫本身的离线测试为 `uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p test_live_probe_helpers.py`，不加载凭据或调用网络。

## 文档与边界

Rust comments/docs 与 Python docstrings 使用简洁 English；重点解释协议、安全、资源和失败边界。Markdown 需检查相对链接、锚点、示例与当前 target 一致；结构性检查不证明行为改善。

不修改 `.env`、私人 `config/`、OAuth 文件，也不读取外部应用认证缓存。保留外部资料的版本、许可与 attribution；历史证据只能按当时边界解释。日志和诊断不能回显真实秘密或私有 payload。

完成时区分静态检查、Rust tests、固定 SDK/loopback、真实 Provider、负载和生产验证。当前正常基线没有任何真实 Provider、付费 API、部署或外部发布。
