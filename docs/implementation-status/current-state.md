# 当前实现

本文只记录当前 checkout 的**实现进度、源码 owner 与确定性证据入口**。实现细节由代码与模块注释拥有，跨模块数据流与模块地图见[当前架构](current-architecture.md)；未实现与未验证范围见[当前状态边界](current-boundaries.md)；Provider 接入进度与未证明边界见[providers/](providers/README.md)；带日期的真实 Provider、SDK 或 Agent 记录见[evidence](evidence/README.md)。

## 实现进度

| 功能域 | 状态 | 主要 owner | 确定性入口 |
|---|---|---|---|
| 网关入口与认证（含 MCP dual-era） | 已实现 | `src/ingress/`、`src/registry/public_model/` | `tests/ingress_contract.rs`、`tests/downstream_auth_contract.rs`、`tests/mcp_contract.rs`、`tests/mcp_dual_era.rs` |
| Bootstrap、用户、上游凭证与静态注册 | 已实现 | `src/config/`、`src/identity.rs`、`src/upstream_credentials/`、`src/credential/`、`src/models/`、`src/providers/`、`src/registry/` | `tests/config_contract.rs`、`tests/example_config.rs`、`tests/upstream_credential_config.rs`、`tests/startup_contract.rs` |
| OAuth 登录与运行时刷新 | 已实现 | `src/oauth2_credentials/` | `tests/oauth2_login_cli.rs` |
| Models API 与请求预检 | 已实现 | `src/pipeline/generation/`、`src/pipeline/embeddings/`、`src/registry/public_model/` | `tests/forwarding_contract.rs`、`tests/ingress_contract.rs`、`tests/provider_boundary_contract.rs` |
| Generation Native 与 Protocol Bridge（Static/Event IR） | 已实现 | `src/ir/generation/`、`src/bridge/`、`src/pipeline/generation/`、`src/provider/`、`src/transport/` | `tests/generation_ir_*_contract.rs`、`tests/bridge_conversion_contract.rs`、`tests/forwarding_contract.rs`、`tests/sse_contract.rs`、`tests/process_replay_contract.rs` |
| Retry、fallback、cooldown 与取消 | 已实现 | `src/ingress/forwarding/`、`src/execution/`、`src/ingress/health.rs`、`src/ingress/streaming/` | `tests/forwarding_contract/resilience.rs`、`tests/process_replay_contract.rs` |
| Embeddings | 已实现（单 Route Native） | `src/pipeline/embeddings/` | `tests/embedding_forwarding_contract.rs` |
| Native 图片/文件/音频输入 | 已实现（按 provider 页收窄） | `src/providers/*/`、`src/pipeline/generation/` | `tests/forwarding_contract.rs`、`tests/forwarding_contract/file_input.rs` |
| Images Generations | 已实现（单 attempt） | `src/pipeline/images/`、`src/ingress/forwarding/images.rs` | `tests/images_forwarding_contract.rs` |
| 管理员 probe | 已实现（单 case） | `src/probe.rs`、`src/probe/`、`src/bin/openbridge-probe.rs` | `src/probe/tests.rs` |
| 观测与测试资产 | 已实现 | `src/observability/`、`testdata/`、`tools/corpus/` | `tests/observability_contract.rs`、`tests/otlp_trace_contract.rs`、`tools/corpus/tests/` |

## 模型与 Provider 接入

当前 Model、Provider Target、候选顺序和 Public Model 关系见[Model 与 Provider 映射](model-provider-mapping.md)；各 Provider 的接入进度与未证明边界见[providers/](providers/README.md)。运行时可见性受 active credential pool 收窄；静态映射不表示实时可达、账号 entitlement 或真实 Provider 验收。

## 最近确定性验证

- 2026-08-31 当前 checkout 通过 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`、`git diff --check`、Python corpus tests 与 corpus lint。
- 2026-08-31 有界管理员 probe 覆盖 DeepSeek、MiMo 与 GLM Chat，以及 Bailian DeepSeek V4 Flash 与 Zhipu GLM-5.3 Responses JSON/SSE；synthetic-user production Router 覆盖四家 Chat JSON/SSE。
- 2026-09-01 nullable Chat usage detail 修复、无状态 function-tool probe 扩展（28 个独立首轮请求）与 probe unit-case + 固定 inline PNG case 均通过完整基线与静态扫描。
- 2026-09-01 MiMo-V2.5 Chat JSON Object 完成管理员 probe 与真实下游 Gateway JSON/SSE 验收（64-token 上限）。
- 2026-09-02 probe 接受管理员自定义 `--prompt`（≤ 4 KiB，非 tool case）与 `--schema`/`--schema-name`（≤ 8 KiB JSON object，仅 JSON Schema case）两个有界覆盖维度：自定义 `--schema` 的 case 恒为 `inconclusive`，报告记录覆盖内容指纹（SHA-256 前 16 位）而不保留原文；无覆盖时全部 canonical case 保持不变。
- 2026-09-02 probe 以 Responses 协议为基准新增三个 Responses-only 单字段差分 case：`reasoning-summary`（`summary:"auto"` + 非空 summary 观测）、`include-encrypted-content`、`prompt-cache-key`；Generation case 总数 22，完整基线与静态扫描通过。
- 2026-09-02 `tools/probe/matrix.py` 编排 4 Target × 双协议 × 双交付 × 22 case 共 328 次真实探测（deepseek-v4-flash-vision-exp、mimo-v2.5、glm-5.3-flash、qwen3.8-max），结果与差异记录见 [evidence](evidence/2026-09-02-dual-protocol-capability-matrix.md)；原始报告在 `testdata/runtime/probe-2026-09-02/`（gitignored，不入库）。

以上只覆盖单一账号/模型与固定 payload，不替代 live Bridge、外部 SDK/Agent、Responses production Router、负载或长期运行验证；真实外部记录以 [evidence](evidence/README.md) 为准。
