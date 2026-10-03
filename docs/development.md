# 开发指南

开发对象为 Rust 语义库、凭据组件、最小 loopback 网关与独立离线验收。启动方式见 [HTTP 指南](http-gateway.md)；真实 Provider probe 不属于默认验证。

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
| `semantic` | `tests/semantic/`：纯语义、codec/lowering、adapter 隔离、编辑失效与 replay 保真；纯 codec 预期不依赖产品 catalog，绑定准入另行检查 |
| `credential` | `src/credential/` 的 manager/driver/store 单测与 `tests/credential.rs` 的独立 CLI/进程边界；只用 synthetic authority/账户文件，不依赖 Gateway。签名 fixtures 均为本地合成密钥 |
| `transport` | `tests/transport/`：framing、Responses/Chat SSE、Chat envelope、body lifecycle；基础 framer、增量 Attempt/ResponseDelivery、实际 I/O commit 边界和 synthetic body I/O 各自验证 |
| `gateway` | 一个真实 Router→synthetic HTTP Provider smoke；另一个隔离环境 binary bootstrap gate，使用 synthetic keys 与拒绝出站的 loopback 代理，不调用真实 Provider |
| `sdk_loopback` | 显式 ignored 的固定 Python SDK codec fixture gates，以及真实 Gateway Router→synthetic Provider 的双协议 JSON/SSE 续轮 gate，不进入默认外部依赖检查 |

`tests/support/` 只共享必要的 synthetic builders 和独立 wire 预期，不从被测 encoder 生成 oracle。测试按责任层组织：凭据匹配检查不构造整座 Gateway，codec 检查不查询模型 catalog，候选/route 检查归 execution/topology。保留默认 topology 的编译检查，不复制逐模型清单。删减重复 smoke 或快照前，确认独立的语义、安全、资源和交付反例仍有覆盖；不以测试数量为目标。

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

## 测试语言与 JS 工具

IR、codec/lowering、资源和生命周期测试保留 Rust，直接验证真实类型和 owner；独立 wire 预期不由被测 encoder 生成。Python 验证 Python SDK 的解析/派生视图与 probe 控制层；JS/pi 消费端使用 TS。不要为跨语言测试增加第二套 IR 或预算状态机。CLI 黑盒仍在 Cargo 默认 gate，不能因迁移丢出常规检查。

Node 版本和 TS/pi 类型依赖由根 [package.json](../package.json) 与锁文件固定；首次准备依赖：

```sh
npm ci --ignore-scripts --no-audit --no-fund --engine-strict
npm run typecheck
npm test
```

已有缓存时可给 `npm ci` 加 `--offline`。Node 原生执行 TS 不等于类型检查，两个命令均需通过。Node 单测只用 synthetic 数据，不导入 pi runtime 或联网；实际 pi synthetic/live gate 仍须显式运行。更改工具或锁文件必须进入 probe 源码指纹。子进程有界收集 stdout/stderr，失败时终止并回收，不以日志缓冲代替 deadline。

## 固定 OpenAI SDK loopback

Python 版本由 `tests/sdk/.python-version` 固定；OpenAI SDK 与测试环境 pip 在 `tests/sdk/pyproject.toml` 声明，全部传递依赖和下载 hash 由 `tests/sdk/uv.lock` 固定。环境只安装到被忽略的 `tests/sdk/.venv/`，不向系统 Python 安装 pip/package。不要直接 `pip install -U` 让环境偏离锁文件。

首次准备需要依赖下载；已有缓存可为 sync 加 `--offline`：

```sh
uv sync --project tests/sdk --locked
uv run --project tests/sdk --locked --offline python -m pip check
uv run --project tests/sdk --locked --offline cargo test --locked --offline --test sdk_loopback -- --ignored --test-threads=1
```

[SDK target](../tests/sdk_loopback.rs)包含 codec fixture 和真实 Gateway→synthetic Provider 两类边界。前者检验固定 SDK 对完整 JSON/SSE、工具/derived view/opaque history 的消费与回放；后者检验认证、可信模型绑定、预算、实际 HTTP body 和续轮接线。预期 wire 独立编写，不由被测 encoder 或模型正文推导，也不从缺失报告补造 SDK 字段。

SDK 场景使用不会被 Python 优化模式移除的显式检查；普通/优化模式的错误结果反例归 Python 单测。所有 listener 都是临时 literal loopback，凭据与工具结果均为 synthetic；禁止环境代理、真实 Provider、自动重试和私有配置读取。进程必须有 deadline 与取消清理。这些 gates 不证明一般 SDK/Agent、网络、负载或生产兼容性；更细的场景以 owning tests 为准，不在指南复制矩阵。

## 受控真实 Provider gates

统一入口、计划、共享账本、预算、原调用诊断和具名场景归 [probe 指南](probes.md)。真实调用必须另行授权目标、矩阵、请求/token 预算和脱敏范围；已有凭据、计划或成功记录不授权重跑。

离线回放仅处理明确指定的获准 capture，不读凭据、不联网，也不把同源 capture 当作独立 fixture。使用入口前检查 [library probe](../examples/live_probe.rs) 的 replay 分支或 [replay CLI](../examples/replay_chat.rs) 的实际参数范围。原始正文捕获默认关闭；诊断和报告只保存白名单元数据，不能记录 credentials、正文或 opaque 值。

## pi 探测诊断守卫

[正文观察器](../examples/provider_probe_observation.ts)只在内存中比较同一次响应，输出有界分类；[header 守卫](../examples/probe_http_headers.ts)不修补客户端认证。独立 Node 防线与显式 pi 运行方式见 [probe 指南](probes.md#离线验证)。客户端认证拒绝与正确认证后的出站拒绝是不同边界，不能用注入正确 token 的中继掩盖错误。

## 文档与边界

Rust comments/docs 与 Python docstrings 使用简洁 English；将协议、安全、资源和失败不变量、非显然兼容理由及必要来源 URL 放在 owning code 旁，不抄测试结果或模型清单。Markdown 保留稳定决策、跨模块合同、设计缺口和操作方法，不保留历史分析、审计报告或任何测试结果（包括离线/SDK）；按 [AGENTS 查询流程](../AGENTS.md#current-provider-model-and-compatibility-information)获取动态信息。检查相对链接、锚点、占位符示例和规则一致性；结构性检查不证明行为改善。

默认检查不修改 `.env`、私人 `config/` 或 OAuth 文件，不读取外部应用认证缓存。保留仍使用来源的版本、许可与 attribution；历史查 Git。日志和诊断不能回显秘密或私有 payload。

完成时区分静态检查、Rust tests、固定 SDK/loopback、真实 Provider、负载和生产验证。当前正常基线没有任何真实 Provider、付费 API、部署或外部发布。
