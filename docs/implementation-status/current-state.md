# 当前实现

本文只记录当前 checkout 的 executable scope、源码 owner、确定性测试入口和外部验收入口。实现细节由代码与模块注释拥有，跨模块数据流与模块地图见[当前架构](../architecture.md)；实现限制和未验证边界见[当前状态边界](current-boundaries.md)；Provider 特有边界见[Provider 接入进度](providers/README.md)；固定日期的外部观察见[evidence](evidence/README.md)。

## 1. 当前实现范围

| 功能域 | 当前范围 | 主要 owner |
|---|---|---|
| 网关入口、Bearer 认证与 MCP dual-era | Chat Completions、Responses、Models、扩展 Models、Embeddings、Images Generations 和本地 `hello` MCP 入口 | `src/ingress/`、`src/registry/public_model/`、`src/mcp/` |
| Bootstrap、用户、上游凭证与静态注册 | 严格启动解析、用户认证、API-key/OAuth binding、canonical Model、Provider Target、Route 与 Public Model 编译 | `src/config/`、`src/identity.rs`、`src/credential/`、`src/upstream_credentials/`、`src/oauth2_credentials/`、`src/models/`、`src/providers/`、`src/registry/` |
| Generation | 统一 Static/Event IR decode/encode；Native 结果完整性、有界 SSE 规范化、封闭集合 Bridge、工具与 structured-output 固定预检 | `src/ir/generation/`、`src/bridge/`、`src/pipeline/generation/`、`src/provider/`、`src/transport/` |
| resilience 与 body lifecycle | 固定 Route 顺序、有限 retry/fallback、credential rotation、单进程 cooldown、取消、SSE 终态与有界 body 处理 | `src/ingress/forwarding/`、`src/execution/`、`src/ingress/health.rs`、`src/ingress/streaming/` |
| Embeddings | 单 Route Native execution，含输入、encoding、dimension 和 batch limit 预检 | `src/pipeline/embeddings/` |
| 图片、文件和音频 | 按 Provider/任务注册的 Native surface；Images Generations 仅同步单 attempt JSON URL | `src/providers/*/`、`src/pipeline/images/`、`src/ingress/forwarding/images.rs` |
| OAuth 与观测 | ChatGPT/Grok 订阅 OAuth 生命周期；本地 JSONL content snapshot 与 OTLP/HTTP traces/metrics | `src/oauth2_credentials/`、`src/observability/` |

当前 Model、Provider Target、候选顺序和 Public Model 关系只见[映射](model-provider-mapping.md)。静态映射和 `/v1/models` 都不表示
credential 有效、账号 entitlement、Provider 可达、配额或真实模型质量。

## 2. 确定性测试入口

这些是验证当前实现机制的入口，不是本页对最近一次运行结果的声明；结果应以实际命令输出、CI 或带日期 evidence 为准。

| 边界 | 入口 |
|---|---|
| ingress、认证、MCP、启动与配置 | `tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs`、`tests/mcp_contract.rs`、`tests/mcp_dual_era.rs`、`tests/config_contract.rs`、`tests/example_config.rs`、`tests/startup_contract.rs` |
| registry、Models API、能力预检与 Provider contract | `tests/provider_boundary_contract.rs`、`tests/provider_contract.rs`、`tests/forwarding_contract.rs` |
| Generation IR、Bridge、JSON/SSE 与 replay | `tests/generation_ir_*_contract.rs`、`tests/bridge_conversion_contract.rs`、`tests/sse_contract.rs`、`tests/process_replay_contract.rs`、`tests/catalog_replay_contract.rs` |
| retry/fallback/cooldown、取消与资源边界 | `tests/forwarding_contract/resilience.rs`、`tests/process_replay_contract.rs` |
| Embeddings、Images 和媒体输入 | `tests/embedding_forwarding_contract.rs`、`tests/images_forwarding_contract.rs`、`tests/forwarding_contract/file_input.rs` |
| 管理员 probe unit cases | `src/probe/tests.rs`、`src/probe/`、`src/bin/openbridge-probe.rs` |
| OAuth、观测、OTLP 与 corpus | `tests/oauth2_login_cli.rs`、`tests/upstream_credential_config.rs`、`tests/observability_contract.rs`、`tests/otlp_trace_contract.rs`、`tools/corpus/tests/` |
| semantic Router 回归 | `tests/semantic_router_contract.rs`；方法和刻意收窄范围见 [`testdata/semantic-testing.md`](../../testdata/semantic-testing.md) |

验证命令与分层规则见[开发指南](../development.md)。确定性测试不证明真实 Provider、外部 SDK/Agent、负载、长期运行或生产兼容。

## 3. 外部验收入口

固定 OpenAI Python SDK 的 Native Responses JSON/SSE 工具续轮已有显式 loopback gate：`tests/openai_responses_sdk_loopback.rs`，默认 ignored；[执行证据](evidence/2026-09-09-openai-responses-sdk-loopback.md)只证明对应 SDK 与 synthetic Router 路径，不证明真实 Provider 工具质量。


外部记录固定于当时的 checkout、账号、区域、网络、endpoint、model ID 和 payload；它们不承担当前能力所有权。

具体记录及其覆盖范围由[evidence 索引](evidence/README.md)维护；各 Provider 页只解释与当前接入相关的证据和未验证边界，本页不复制记录清单。

管理员 probe 的普通执行结果不自动生成独立记录；只有具有独立接入验收价值或观察到与引用来源不一致的差异时，才进入[evidence 索引](evidence/README.md)。
