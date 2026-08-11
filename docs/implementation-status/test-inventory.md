# 当前测试资产与保留标准

## 状态

**Confirmed。** 当前 checkout 收集 291 个 Rust 默认测试和 45 个 Python testkit 测试。Rust 没有 ignored test；
`testdata/cases/` 的 51 个 canonical wire case 与 `testdata/semantic-cases/` 的 9 个 semantic case 是输入/oracle，
不计入可执行测试。

2026-08-11 完成一次测试质量收敛：删除 142 个只固定内部 capability DTO/交集、完整模型目录、Route ID/顺序、候选数量或
规划中间态的 Rust 测试。产品实现未因此改变。

## 保留门槛

默认测试必须至少保护以下一项：

1. 客户端可观察的 HTTP、JSON、SSE、CLI 或进程启动结果；
2. Provider adapter 产生的受信相对 URI、header、请求 body 或响应终态；
3. Chat ↔ Responses、Embeddings 或 SSE 的协议转换与错误边界；
4. 认证、凭证、敏感信息、body budget、retry/fallback/cooldown、取消或 observability 的运行时安全结果；
5. 直接影响启动或请求安全的 fail-closed 边界，例如非法 registry 引用、endpoint、credential ownership 或 body budget。

不再为以下内容单独建立测试：

- 完整 canonical Model/Target/Public Model 清单和静态计数；
- 某个模型的完整 capability JSON 快照；
- Route ID、Route 数量、候选数量或候选顺序；
- 仅调用 planner/compiler 后检查内部 DTO、交集结果或中间态；
- 已由 HTTP/forwarding/bridge 测试覆盖的重复 preflight 正反例。

fail-closed 测试必须从 registry 构建、HTTP admission、Provider wire 或资源生命周期入口验证结果，不单独测试 capability 数据类型的
集合、交集或构造器中间态。

## Rust 业务测试入口

| 业务边界 | 主要测试入口 |
|---|---|
| HTTP admission、认证与启动 | `tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs`、`tests/startup_contract.rs` |
| Bootstrap、用户与凭证 | `tests/config_contract.rs`、`tests/upstream_credential_config.rs`、`tests/credential_store_contract.rs`、`tests/example_config.rs` |
| Native/Provider 转发 | `tests/forwarding_contract.rs`、`tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` |
| Protocol Bridge 与 SSE | `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs`、`tests/protocol_bridge_replay.rs`、`tests/sse_contract.rs` |
| Embeddings | `tests/embedding_forwarding_contract.rs` |
| Retry/fallback/cancel | `tests/forwarding_contract/resilience.rs`、`tests/process_replay_contract.rs` |
| OAuth2 | `src/oauth2_credentials/**/tests.rs`、`tests/oauth2_login_cli.rs`、`tests/forwarding_contract/chatgpt.rs` |
| Probe 与 transport | `src/probe/tests.rs`、`src/transport/upstream.rs`、`src/bin/openbridge-probe.rs` |
| Observability | `tests/observability_contract.rs`、`tests/otlp_metrics_contract.rs`、`tests/otlp_trace_contract.rs` |
| MCP | `tests/mcp_contract.rs` |

`tests/example_config.rs` 只验证两个 checked-in Bootstrap profile 可以编译成运行时注册表，不再复制模型、能力或路由目录。
标准/扩展 Models 的业务契约由 `tests/forwarding_contract/models.rs` 从 HTTP 边界验证。

## 已删除的低价值套件

- `tests/capability_definition_contract.rs`
- `tests/native_routing_contract.rs`
- `tests/embedding_definition_contract.rs`
- `tests/embedding_registry_contract.rs`
- `tests/qwen36_registry_contract.rs`
- `tests/example_config/configuration.rs`
- `tests/example_config/providers.rs`
- `tests/example_config/routing.rs`
- `src/providers/catalog/route_compiler.rs` 中的固定 Route 顺序/自动补桥单测
- `src/registry/public_model/compiler/aggregate.rs` 中的 capability 交集单点断言
- `src/core/capability/generation.rs` 中的 capability 集合、交集和构造器断言
- `src/pipeline/analysis/generation.rs` 中的 structured-output 中间态合并断言
- `tests/provider_contract.rs` 中的全 Provider operation/capability 静态矩阵

## Canonical oracle 与 Python testkit

`testdata/cases/` 保留 19 个 Bridge、10 个 fault、20 个 Native 和 2 个 transport case；
`testdata/semantic-cases/` 保留 9 个 function-tool semantic case。它们是 Rust replay 与 Python testkit 共享的只读协议输入，
fixture 存在本身不证明 OpenBridge runtime 已执行该行为。

Python 的 45 个测试继续负责 corpus schema/provenance/secret lint、确定性 generation/report/pack、SSE fragmentation、
Mock Server/Client loopback、observation verifier 和 normalized function-tool semantic verifier。本轮没有修改 Python 或 `testdata/`。

## 验证证据与边界

2026-08-11 当前 Windows checkout 执行：

```powershell
cargo test --locked -- --list
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

Rust 收集并通过 291 个测试；OTLP trace collector 的业务测试改为按通知等待两个已解码 span，定向连续运行 10 次并通过，随后完整
Rust baseline 通过。Python/testdata 未变化，因此没有为本次 Rust 测试治理重复运行 Python基线。默认测试仍不包含外部 OpenAI SDK、
Codex、Hermes、真实 Provider、负载或长期运行验收。

## 维护规则

1. 新测试必须在名称、注释或 fixture 中明确客户端结果、wire 行为或安全失败边界；不得只复制注册表事实。
2. 路由行为通过实际 fallback/retry/bridge 结果验证，不断言 Route ID、候选数量或顺序。
3. Models/能力通过 HTTP list/retrieve、请求接受/拒绝和实际 egress 验证，不维护完整静态快照测试。
4. `testdata/` 或 `tools/corpus/` 变化时，仍同步更新[协议测试语料与工具](protocol-test-corpus.md)并运行 Python 基线。

## 相关文档

- [实施现状目录](README.md)
- [协议测试语料与工具](protocol-test-corpus.md)
- [TDD 与证据要求](../functional-requirements/delivery-and-evidence.md)
