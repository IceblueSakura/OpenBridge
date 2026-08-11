# 测试资产与保留标准

## 状态与所有权

**已确认。** 当前默认 Rust 测试覆盖网关运行时行为；Python testkit、canonical wire/semantic case 和派生 variant 的
版本与数量只由 [协议语料与工具](protocol-corpus.md)维护。本页不保存会随测试增删漂移的总数，也不保留已删除测试清单。

测试文件存在只证明有对应资产；只有实际运行结果才能形成验证证据。Rust、Python loopback、外部 SDK、目标 Agent、真实
Provider、负载和长期运行分别属于不同证据层。

## 保留门槛

默认测试至少保护以下一项：

1. 客户端可观察的 HTTP、JSON、SSE、CLI 或进程启动结果；
2. Provider adapter 产生的受信相对 URI、header、请求 body 或响应终态；
3. Chat ↔ Responses、Embeddings 或 SSE 的协议转换与错误边界；
4. 认证、凭证、敏感信息、body budget、retry/fallback/cooldown、取消或 observability 的运行时安全结果；
5. 直接影响启动或请求安全的 fail-closed 边界，例如非法 registry 引用、endpoint、credential ownership 或 body budget。

不为以下内容单独建立测试：

- 完整 canonical Model、Target 或 Public Model 清单及静态计数；
- 单个模型的完整 capability JSON 快照；
- Route ID、Route 数量、候选数量或候选顺序；
- 只调用 planner/compiler 后检查内部 DTO、交集结果或中间态；
- 已由 HTTP、forwarding 或 Bridge 测试覆盖的重复 preflight 正反例。

Fail-closed 行为从 registry 构建、HTTP admission、Provider wire 或资源生命周期入口验证；集合、交集和构造器中间态不作为
独立产品合同。

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
| MCP | `tests/mcp_contract.rs`、`tests/mcp_dual_era.rs` |

`tests/example_config.rs` 只证明两个 checked-in Bootstrap profile 可以编译为运行时注册表；标准/扩展 Models 的客户端
合同由 `tests/forwarding_contract/models.rs` 从 HTTP 边界验证。

## Canonical oracle 与 Python testkit

`testdata/cases/` 与 `testdata/semantic-cases/` 是 Rust replay 与 Python testkit 共享的只读输入/oracle。fixture 存在本身
不证明 OpenBridge runtime 已执行该行为。Python 负责 corpus schema/provenance/secret lint、确定性
generation/report/pack、SSE fragmentation、Mock Server/Client loopback、observation verifier 和 normalized function-tool
semantic verifier；OpenBridge 路由、retry/fallback/cooldown 和取消策略仍由 Rust 测试拥有。

## 验证入口

```powershell
cargo test --locked -- --list
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

修改 `testdata/` 或 `tools/corpus/` 时追加 [协议语料与工具](protocol-corpus.md) 中的 Python 基线。默认基线不包含
外部 SDK、Codex、Hermes、真实 Provider、负载或长期运行验收。

## 维护规则

1. 新测试必须明确客户端结果、wire 行为或安全失败边界，不复制注册表事实。
2. 路由行为通过实际 fallback/retry/Bridge 结果验证，不断言内部 Route ID、候选数量或顺序。
3. Models/能力通过 HTTP list/retrieve、请求接受/拒绝和实际 egress 验证，不维护完整静态快照。
4. corpus/testkit 的版本、case、variant 和 pytest 数量只在 `protocol-corpus.md` 更新。

## 相关文档

- [实施现状目录](../README.md)
- [协议语料与工具](protocol-corpus.md)
- [文档维护与证据治理](../../README.md)
