# 外部测试资产登记表

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | 各资产的事实由"Owner 文档"列链接的叶文档与综合文档维护；本页不另行拥有快照 |
| Last reverified | 2026-09-01，仅本地导航整理；没有重新运行或重新固定任何外部资产 |
| Scope | 登记外部协议测试、框架测试、可吸收场景清单与语义评测基准，供吸收决策检索 |
| Evidence boundary | 本页不拥有事实，不评价资产质量，不构成采用承诺；资产的覆盖、边界与许可证结论以 owner 文档为准 |
| Recheck trigger | owner 文档更新、资产版本/许可证变化、新的外部测试资产被登记，或某次采用决定落地 |

## 采用义务（所有资产通用）

- 证据类型按 [docs/README.md 证据表达](../../README.md#6-证据表达) 的七层分级标注；低层证据不能替代高层验收。
- 复制代码、payload 或 fixture 前必须核对具体文件的 license、来源和 attribution；默认只借鉴独立场景并自主编写 synthetic fixture。
- 每次采用决定（复制 / 重写场景 / 只读参考）完成后，在本表"状态"列记录去向，避免重复调研。

## 1. 协议与兼容性测试资产

| 资产 | Owner 文档 | 证据类型 | 固定基线 | 覆盖角色 | 许可证与复制边界 |
|---|---|---|---|---|---|
| gpt-oss compatibility-test | [调研](../openai/gpt-oss-compatibility-test-analysis.md) | 外部 SDK + 真实/兼容模型 | 未固定 commit（2026-07-26 在线复核） | API-shape 与基本 function calling smoke | 采用/复制前必须重新 pin 版本并核对许可证 |
| OpenAI SDK streaming consumers | [调研](../openai/openai-sdk-stream-test-assets-analysis.md) | SDK consumer 合同 | 默认分支（2026-07-26 在线复核） | 客户端如何消费 Chat SSE 增量、accumulator 所需字段 | 使用时必须固定 SDK 版本 |
| Open Responses Compliance | [调研](../openai/open-responses-compliance-analysis.md) | 黑盒 acceptance（HTTP/SSE/WebSocket 场景） | 未固定 commit（2026-07-26 在线复核） | Responses schema、terminal 与 continuation；独立规范，不等于官方 API | 采用/复制前必须重新 pin 版本并核对许可证 |
| Codex Responses/tool tests | [调研](../codex/codex-protocol-test-assets-analysis.md) | 确定性 Rust fixture | `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff` | Responses 消费侧、tool lifecycle、`call_id` 匹配、并行时序 | Apache-2.0；重写场景也应记录原测试路径 |
| LiteLLM Responses/转换 tests 与 issues | [调研](../litellm/litellm-protocol-test-assets-analysis.md) | 项目内部测试 + 真实缺陷回归 | `23de7a15d9d40006ee596e617475ba101d60c5e9` | 多 Provider 字段差异、负面回归样本（call_id 状态丢失、content block 切换） | MIT；`enterprise/` 另有条款，不复制大段 transcript |

覆盖缺口与跨资产结论见 [测试资产综合调研](../cross-project/chat-responses-sse-tool-test-suite-survey.md)。

## 2. 框架测试与可吸收场景清单

下列条目记录的是"已调研的可吸收测试场景"，吸收时按场景重写，不复制原仓库代码；吸收清单的具体条目在 owner 文档内。

| 资产 | Owner 文档 | 场景清单位置 | 许可证与复制边界 |
|---|---|---|---|
| Bifrost deterministic 场景 | [调研](../protocol-gateways/bifrost.md) | [§6 可吸收测试资产](../protocol-gateways/bifrost.md#6-可吸收测试资产) | Apache-2.0；重写也记录原测试路径与 commit |
| Helicone routing/retry/cache 场景 | [调研](../protocol-gateways/helicone.md) | [§6 可吸收测试资产](../protocol-gateways/helicone.md#6-可吸收测试资产) | GPL-3.0；默认只借鉴测试形状，复制需单独审查 |
| Portkey adapter 边界场景 | [调研](../protocol-gateways/portkey.md) | [§6 可吸收测试资产](../protocol-gateways/portkey.md#6-可吸收测试资产) | MIT；选最小场景自主编写 |
| TensorZero capability/tool 场景 | [调研](../protocol-gateways/tensorzero.md) | [§7 可吸收测试资产](../protocol-gateways/tensorzero.md#7-可吸收测试资产) | Apache-2.0；E2E 依赖真实 Provider，只提炼独立语义 |
| Vercel AI SDK 类型目录场景 | [调研](../protocol-gateways/vercel-ai-sdk.md) | [§6 可吸收测试资产](../protocol-gateways/vercel-ai-sdk.md#6-可吸收测试资产) | Apache-2.0；不复制 API key、snapshot 噪声与完整 harness |
| LiteLLM server-tool 回归场景 | [调研](../litellm/litellm-ir-server-tool-regressions-analysis.md) | [§6 Edge cases 与测试资产](../litellm/litellm-ir-server-tool-regressions-analysis.md#6-edge-cases-与测试资产) | MIT；`enterprise/` 另有条款，默认自主写 fixture |
| new-api converter tests | [调研](../new-api/new-api-request-conversion-analysis.md) | [§14 测试资产](../new-api/new-api-request-conversion-analysis.md#14-测试资产) | AGPL-3.0；含已执行的本地运行记录，不复制其测试数据 |
| 统一吸收清单（按机制分类） | [综合](../cross-project/protocol-ir-ecosystem-analysis.md) | [§8 测试吸收清单](../cross-project/protocol-ir-ecosystem-analysis.md#8-测试吸收清单) | 各前置项目许可证见综合 §9 |

## 3. 语义评测基准

| 基准 | Owner 文档 | 覆盖角色 | 许可证与复制边界 |
|---|---|---|---|
| RULER / NoLiMa / LongMemEval / LongBench v2 / BFCL V4 / ToolSandbox / JSONSchemaBench | [Semantic evaluation methods](../semantic-testing-methods.md) | 长上下文、function-tool、stateful tool、structured-output 的任务设计 | NoLiMa 禁止商业使用；多项组合第三方数据，默认只借鉴任务结构并自写 synthetic case |

## 4. 状态与去向记录

| 日期 | 资产 | 采用决定 | 去向（本地位置） |
|---|---|---|---|
| — | — | — | 尚无吸收记录；采用后在此登记 |
