# 开发指南

本文面向维护者，定义变更流程、测试职责、验证入口与交付条件。产品行为由[功能需求](functional-requirements/README.md)定义，模块关系见[当前架构](architecture.md)，Agent 的授权和安全约束见[AGENTS.md](../AGENTS.md)。

## 开始一项变更

1. 检查当前分支、`git status` 和目标文件已有 diff，保留无关工作。
2. 阅读对应需求、源码和测试；跨模块工作同时阅读架构。状态页只用于寻找差距和证据，不能代替源码检查。
3. 明确用户要求的结果、非目标和验证边界。源码与产品契约冲突时先判断是实现缺陷、文档过时还是待裁决行为，不能自动以其中一方覆盖另一方。
4. 行为变更前，在[当前开发焦点](implementation-plans/current-focus.md)记录用户已批准的范围、关联需求、失败测试及验收边界。该文件是范围记录，不是独立授权来源。
5. 先以失败测试或最小复现确定行为，再实施最小充分修改；优先保护独立机制，而不是增加重复场景。
6. 运行 focused validation，再执行与影响面相称的基线。同步受影响的契约、serialization、OpenAPI、示例、fixture 和测试。
7. 更新发生变化的当前事实和证据指针，在交付中报告实际验证结果；完成行为切片后将当前焦点恢复为空。不累积完成日志。

纯文档、注释或指引维护不制造行为焦点。未发布原型可在获准范围内直接替换 API 或 Bootstrap 字段，不保留无意义的 alias、兼容垫片或双实现；这不授权修改私有配置。

## 按修改范围选择验证

先运行直接保护改动行为的测试。以下命令在仓库根目录执行；它们是验证入口，不表示当前 checkout 已执行通过。

### Rust 行为与依赖

```sh
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

依赖变更必须同步 `Cargo.toml` 与 `Cargo.lock`，并重新执行 locked 验证。编译或格式检查不能代替行为测试。

### Bootstrap logging、正文生命周期与观测隔离

先执行：

```sh
cargo test --locked --test config_contract
cargo test --locked --test example_config
cargo test --locked --test observability_contract
cargo test --locked --test otlp_trace_contract
```

这些测试保护开发模板与省略字段默认值、认证后的 snapshot、脱敏、有界捕获和 OTLP 隔离；不能证明生产日志保留策略、磁盘故障或长期负载。

### Corpus 与测试工具

修改 canonical `testdata/` 契约或 `tools/corpus/` 行为时追加：

```sh
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

数据格式和发布规则见 [testdata README](../testdata/README.md)，语义测试流程与独立验收边界见 [semantic testing](../testdata/semantic-testing.md)。只修改这些目录中的说明文档时仍按文档验证处理，不因目录名称自动运行全部测试。

### 文档与指引

- 检查 Markdown 文件链接、本地锚点、移动后的旧路径和入口可达性。
- 检查规则、示例与事实归属一致，索引不重复维护数量和状态。
- 对行为描述核对对应源码与测试；整理旧证据不刷新其外部复核时间。
- 执行 `git diff --check`。
- 若改变 OpenAPI、运行时嵌入资产、serialization 或产品行为，追加相关 Rust 测试和相称基线。

`docs/openapi.yaml` 与 `docs/swagger-ui.html` 是编译交付的运行时资产，不作为普通文档随意移动。

## 配置与注释约定

`config/bootstrap.toml` 与 `config/bootstrap.example.toml` 是 checked-in 开发模板，必须解析为相同 `BootstrapConfig`。每个赋值紧邻的前一行使用简洁英文注释说明运行时作用。开发模板显式启用 logging 内容开关与省略字段默认为关闭是不同语义，修改时同步配置合同、示例和测试，用户入口保留明确风险提示。

Rust 注释与文档、Python docstring 使用简洁英文。非平凡 Rust 模块通过 `//!` 说明职责和边界，公共 API 使用 `///`；对非显然的安全、协议、并发和清理 helper 解释原因。多阶段函数按逻辑阶段添加简洁的动作说明，不复述显而易见的代码，不留猜测式 TODO 或长篇设计讨论。只更新直接修改面的过时注释。

## 测试职责与最小充分覆盖

Rust tests 保护运行时 registry/routing、Provider wire、retry/fallback/cooldown、取消、Protocol Bridge 和进程内不变量。Python 保护 corpus 完整性、确定性生成/报告/打包、字节级 SSE fragmentation 及 standalone mock/client 行为。

新增测试必须保护独立的客户端结果、Provider wire 或安全/资源失败边界。新 Model、Route 或 Provider 实例本身不构成新增测试的理由：

- 在最低职责层验证一个机制；production Router smoke 只在增加独立价值时追加。
- 不维护完整模型库存、能力快照、Route 数量/顺序或重复的逐模型验收矩阵。
- 跨层共用 canonical fixture，断言贴近行为所属层，不用生产 codec 生成期望值来证明同一 codec 正确。
- 保留认证、凭据所有权、分配上限、协议终态、重试、取消和资源释放的真实 fail-closed 回归。
- 仅并行独立且隔离端口、目录和输出的场景。顺序 recovery 场景串行，使用 readiness/event 和有界超时，不以 sleep 修补竞态。

新 endpoint 可先用 test-only synthetic Provider 或 loopback 建立合同；需要 Router 验收时必须经过真实认证、limit、analysis、planning、transport、renderer、错误和终态观测路径。synthetic Target 不得进入生产 Models 目录，fake 成功不证明真实 Provider、费用或质量。

新客户端验收优先选择 OpenAI SDK、独立 Python 或 curl；Codex/Hermes 等 Agent 只在明确兼容目标下成为验收入口。

## 证据与外部验证

分别说明静态检查、确定性 Rust、Python/loopback、外部 SDK/独立客户端、目标 Agent、真实 Provider，以及负载/长期运行/生产环境。它们不能互相替代；有测试入口不代表已经执行，一次真实请求也不代表长期兼容。

默认验证不调用真实 Provider，不读取 OpenBridge 私有凭据，不启动面向敏感流量的服务。真实计费探测必须先获得明确授权，并确认目标、精确请求矩阵、输出上限及脱敏报告范围；使用方法见 [Provider 探测指南](guides/provider-probing.md)。

具有独立价值的外部接入验收和实测差异保留在 [evidence](implementation-status/evidence/README.md)，由当前状态链接解释适用性。普通本地测试结果在实际交付或已有 CI 中报告，不要求每次验证新增历史文档。外部协议调研属于 [references](references/README.md)，不是本项目执行证据。

## 完成与交付

检查最终 diff，确认未覆盖用户既有修改、未改私有配置、未纳入生成产物或敏感内容。报告：

- 修改的行为或文档职责及关键文件；
- 实际执行的命令与结果；
- 未执行的 SDK、真实 Provider、负载和长期运行层；
- 剩余冲突、验证缺口或用户动作。

文档更新不等于运行时验收通过，Agent 指引的静态一致性也不证明行为改善。提交与推送必须分别得到用户授权。
