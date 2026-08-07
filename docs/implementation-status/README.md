# 实施现状目录

本目录只记录当前 checkout 已经实现，并由代码、测试或明确验证记录支持的事实。每个已完成的功能点使用一个专题文件，专题文件是该功能
的唯一状态来源；未实施的设计、后续设想和外部协议调研分别放在 `implementation-plans/`、`functional-requirements/` 和 `references/`。

## 已完成的功能点

专题页统一使用“状态 → 已完成内容 → 实现边界 → 验证证据 → 未覆盖范围 → 相关文档”的结构，便于区分实现事实和验收结论。

| 功能点 | 专题文件 | 主要证据入口 |
|---|---|---|
| HTTP 网关接口与下游认证 | [gateway-http-api-and-auth.md](features/gateway-http-api-and-auth.md) | `tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs` |
| 启动配置、用户与受信凭证边界 | [startup-configuration-and-credentials.md](features/startup-configuration-and-credentials.md) | `tests/config_contract.rs`、`tests/upstream_credential_config.rs`、`tests/startup_contract.rs` |
| Provider/Model/Target/API/Route/Public Model 注册表 | [provider-registry-and-model-catalog.md](features/provider-registry-and-model-catalog.md) | `tests/native_routing_contract.rs`、`tests/provider*_contract.rs` |
| Models 接口、公共契约与能力预检 | [models-api-and-capability-preflight.md](features/models-api-and-capability-preflight.md) | `tests/native_routing_contract.rs`、`tests/capability_definition_contract.rs` |
| Chat/Responses Native 转发 | [native-generation-forwarding.md](features/native-generation-forwarding.md) | `tests/forwarding_contract.rs`、`tests/sse_contract.rs` |
| `mimo-v2.5` Chat/Responses Native 图片输入 | [native-image-input.md](features/native-image-input.md) | `tests/example_config.rs`、`tests/forwarding_contract.rs` |
| Chat ↔ Responses Protocol Bridge | [protocol-bridge.md](features/protocol-bridge.md) | `tests/bridge_conversion_contract.rs`、`tests/bridge_forwarding_contract.rs` |
| Retry、fallback、credential rotation、cooldown 与取消 | [resilience-retry-fallback-and-cancellation.md](features/resilience-retry-fallback-and-cancellation.md) | `tests/forwarding_contract.rs`、`tests/sse_contract.rs` |
| OpenAI-compatible Embeddings | [embeddings.md](features/embeddings.md) | `tests/embedding_*_contract.rs` |
| ChatGPT OAuth2 生命周期与 Responses 数据面 | [chatgpt-oauth-startup.md](features/chatgpt-oauth-startup.md) | `tests/oauth2_login_cli.rs`、`tests/startup_contract.rs`、`tests/forwarding_contract.rs` |

## 已实现的横向能力

| 功能点 | 状态文档 | 主要证据入口 |
|---|---|---|
| OpenTelemetry traces/metrics 与 OTLP/HTTP 导出 | [telemetry-metrics.md](telemetry-metrics.md) | `tests/observability_contract.rs`、`tests/otlp_trace_contract.rs`、`tests/otlp_metrics_contract.rs` |

## 横向状态文档

| 文档 | 用途 |
|---|---|
| [当前实现总览](current-implementation.md) | 功能页导航、证据层级和未完成范围总览 |
| [当前代码架构](current-architecture.md) | 模块所有权、装配链和请求数据流；不是功能清单 |
| [运行时指标与遥测](telemetry-metrics.md) | OTLP traces/metrics、SDK instruments、属性和生命周期边界 |
| [上游模型发现与能力探测](capability-probing.md) | 显式 target probe 的实现事实和安全边界 |
| [当前测试资产树](test-inventory.md) | 以功能模块归类全部 Rust/Python 可执行测试，并单列 canonical oracle |
| [协议测试语料与工具](protocol-test-corpus.md) | canonical corpus、Python testkit、Mock Server/Client 和 Rust replay 边界 |

## 证据和维护规则

同一事实出现冲突时，按“当前 checkout → 对应确定性测试 → 本目录最近一次实际验证记录”的顺序处理；历史计数和外部观察不得覆盖 live
source。静态源码、确定性 mock/fixture、loopback/独立客户端、外部 SDK、目标 Agent、真实 Provider、负载和长期运行分别是不同证据层，
专题页必须明确写出实际运行和未运行的层次。

新增完成行为时，先为一个可观察功能点建立专题文件，再把目录和相关导航链接同步更新；不要把同一功能重新复制到多个状态页，也不要把
计划或参考资料写成已完成事实。
