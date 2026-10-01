# 开发指南

当前开发对象为 v2 Rust library（semantic、adapter、topology、execution）与独立离线验收。最小 `openbridge` binary 的启动见 [HTTP 网关指南](http-gateway.md)；没有旧网关或 MCP binary；受控 live probe example 不是默认验证入口，旧 corpus/运行配置的使用方式见 [Git 归档](archive.md)，不是当前开发前置条件。

## 变更流程

1. 检查分支、Git status 与目标 diff；保留未提交工作，不 stage/commit/push。
2. 阅读 [v2 设计](architecture-v2/README.md)、受影响的源码/测试与固定协议资料。明确 owner、支持与拒绝边界。
3. 行为变更在 [current-focus](implementation-plans/current-focus.md)维护获准范围、可观察结果、失败用例和验证门槛，随后按失败证据实现。文档维护不制造运行时切片。
4. 测试在最低职责层保护独立语义；codec 的 decode 与 encode 分别使用独立预期，追加 IR 插入、替换、删除及对应失败边界。round trip 不能自证。
5. 检查最终 diff、文档与引用，报告实际检查和未验收范围。按当前合同验收，不以旧版功能对等为门槛；不得以主线定位、删除旧代码或测试通过宣称生产就绪。

## Rust 检查

Rust/Cargo 由根 `rust-toolchain.toml` 固定；rustfmt/clippy 随该工具链安装。不要保留覆盖这个文件的旧目录级 rustup override，也不要为项目更新全局默认工具链。

集成测试按以下入口分工，按职责筛选，不按实现批次增加 binary：

| Target | 模块与边界 |
|---|---|
| `semantic` | `tests/semantic/`：instructions、phase、tools、reasoning、schema、parsed replay、extensions、text profile/events/admission、function events、response、response/history continuation、chat wire/概率 owner、adapter profile 隔离、usage 缺省规则与来源/依赖保真；纯语义与 codec/lowering |
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

`gateway_sdk::sdk_uses_gateway_for_both_protocols_and_deliveries` 使用实际 `Gateway::serve` 与独立 synthetic HTTP Provider。固定 SDK 的 Chat/Responses JSON/SSE 请求经过认证、模型绑定、缺省 token 预算写入 IR、request lowering、真实 HTTP、response/event adapter 与 HTTP body，再回放实际工具/derived views/encrypted reasoning。上游 oracle 独立检查 model 重绑定、默认预算、call ID、原始参数和工具结果；不由网关 handler 直接构造客户端回答。所有凭据为 synthetic，SDK/transport 禁止环境代理继承与重试，进程有 deadline 与取消清理。该 gate 也用完整、独立编写的 Responses envelope 验证空 message owner 与工具调用的 JSON/SSE 消费，不从 Chat 未报告的设置补造 SDK 必填事实。原生 Responses 的空 owner 与独立 call 不声明 membership；这不代表 Chat 分组可无损跨协议交付。Chat→Responses 显式分组的 static 502、SSE late abort 与请求投影 I/O 前拒绝由既有实际 Router smoke 验证；标准分组 carrier 缺失由 [Generation 缺口](implementation-status/generation.md#语义与表示缺口)维护。该 gate 不代替真实 Provider 经新 binary 的验收。

`transport::body_lifecycle` 保护首帧、取消、背压与异常 body；`transport::responses_sse`、`transport::chat` 和 `transport::framing` 分别保护协议 adapter 与共用 framer 的终态。生命周期场景使用 channel/readiness 和有界 timeout，不用 sleep 隐藏竞争。子进程/listener/producer 需要失败路径清理。

## 受控真实 Provider gates

真实调用需当次明确授权目标、矩阵、token/请求数和报告边界；以下入口不属于默认 tests，也不因环境中存在密钥就获准运行。所有自动 live 入口现在要求共享 run，先通过 `examples/probe.py plan` 离线建计划；预算、调度、原调用诊断、当前命令及结果解释统一归 [probe 指南](probes.md)。以下具名入口只负责选择场景，不再拥有独立的进程内调用额度。

- `examples/live_probe.rs`：显式 `OPENBRIDGE_PROBE=1` 与 `OPENBRIDGE_PROBE_RUN`，模型必须属于计划。选择项、native protocol 与预算以该入口代码为准，不依赖默认目标；目录预检另需显式启用并占共享 slot。真实工具输出按完整已交付历史续轮，报告分别检查终态和场景 oracle。
- `OPENBRIDGE_PROBE_REPLAY_DIR=<existing probe directory> cargo run --locked --offline --example live_probe`：仅回放该入口代码选定的旧格式 captures，不是通用 discovery/replay。先读其 replay 分支确认目标/格式；不读取凭据、不联网，不将 captures 作为独立 fixtures。
- `examples/live_gateway_probe.py`：固定场景的 SDK → 临时 binary → Provider 入口；加密状态场景要求首轮真实 token，以及 SDK send 前的 issuer ID/密文保真检查。具体目标和开关读取入口代码，必须属于已授权 run；不能把可读 reasoning 当作加密续轮成功。

- `examples/live_provider_matrix.py`：统一矩阵的具名入口。目标选择、暂停项与工具策略读取源码，先构建当前 binary，再用已授权 run 缩小矩阵；不在文档同步模型列表或临时账号状态。
- `examples/live_nvidia_probe.py`：目标专用的边界场景入口，仍使用共享账本和相同停止规则；读取代码确认其当前目标，不把脚本名或历史运行当产品支持声明。客户端 close 和后续请求成功不能证明 Provider 停算或停止计费。
- `cargo run --locked --offline --example replay_chat -- <provider-id> <authorized-synthetic-capture.sse> [chat|responses]`：对明确给出的有界 capture 做纯离线 intake 和下游 projection；占位符取值与格式限制以入口代码为准。不读取凭据、不联网，不把 captures 当独立 fixture。
- `uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p 'test_*.py'`：既有 replay checker 和新增矩阵请求数/目标/输出预算的离线防线，无真实调用。

这些 live 入口只在进程内读取工作区 API-key 凭据，均无自动重试；不会更改私有文件或加载 Codex/OAuth 引用。库级 probe 的 raw capture 默认关闭，显式 synthetic capture 会移除已知 opaque/credential 字段，不能充当原始 wire oracle；报告与共享账本位于 run 子目录。SDK/binary gate 只保存白名单结果，不保存 body/headers/token。后者通过启动就绪信号获取临时端口，结束/失败均回收 binary。每次重新执行都是新的一批付费调用，不是“免费重跑测试”。执行结果只在当次交付和授权 run 中报告，不写入 Markdown、注释或适配模型表。probe 序列化守卫本身的离线测试为 `uv run --project tests/sdk --locked --offline python -m unittest discover -s tests/sdk -p test_live_probe_helpers.py`，不加载凭据或调用网络。

## pi 探测诊断守卫

`examples/provider_probe_observation.mjs` 对同一次网关 SSE 响应做有界正文/终态摘要，与 pi 消费结果比较；正文仅在有界内存中比较，只向报告输出长度、分类和一致性，不写入正文、reasoning、opaque 值或 headers。它不是新的协议 decoder，不把大小写/标点差异规范化成 oracle 成功。`examples/probe_http_headers.mjs` 保留客户端实际的 Authorization/Content-Type，禁止测试中继用已知正确 token 掩盖错误认证，也不转发其他 headers。

纯 synthetic Node 检查不依赖 pi 安装、凭据或网络：

```sh
node --test tests/sdk/provider_probe_observation.test.mjs tests/sdk/probe_http_headers.test.mjs
```

实际 pi/Router 的执行逻辑现由仓库 `examples/pi_probe.py` / `pi_probe.mjs` 维护，本机配置和凭据不入仓。认证负例使用临时 synthetic run，真实调用与其他入口共享账本；命令见 [probe 指南](probes.md#pi)。错误 key 应在入口得到 401 且上游尝试为零，正常 key 的拒绝出站代理测试则证明准入仍可达；不要把测试代理注入的凭据当作客户端认证成功。

## 文档与边界

Rust comments/docs 与 Python docstrings 使用简洁 English；将协议、安全、资源和失败不变量、非显然兼容理由及必要来源 URL 放在 owning code 旁，不抄测试结果或模型清单。Markdown 保留稳定决策、跨模块合同、设计缺口和操作方法，不保留历史分析、审计报告或任何测试结果（包括离线/SDK）；按 [AGENTS 查询流程](../AGENTS.md#current-provider-model-and-compatibility-information)获取动态信息。检查相对链接、锚点、占位符示例和规则一致性；结构性检查不证明行为改善。

不修改 `.env`、私人 `config/`、OAuth 文件，也不读取外部应用认证缓存。保留仍使用的外部资料的版本、许可与 attribution；历史问题查 Git，不恢复工作区分析报告。日志和诊断不能回显真实秘密或私有 payload。

完成时区分静态检查、Rust tests、固定 SDK/loopback、真实 Provider、负载和生产验证。当前正常基线没有任何真实 Provider、付费 API、部署或外部发布。
