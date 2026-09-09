# 验证证据目录

本目录保存已经执行、带日期且边界明确的外部验证记录。记录按发生时间固定事实，不承担“当前实现”或“当前 Provider 能力”所有权；这些结论由[当前实现](../current-state.md)、[当前状态边界](../current-boundaries.md)和[Provider 分页](../providers/README.md)解释。

证据层必须分开表述：确定性 Rust/Python 测试、loopback 客户端、外部 SDK、目标 Agent、真实 Provider、负载和长期运行互不替代。真实 Provider 记录只证明当时 checkout、账号、网络、固定 endpoint、模型和 payload。

## 记录准入

独立 evidence 仅在以下情形之一成立时新增：

- **接入验收**：新 Provider、Target 或客户端/SDK 接线本身具有独立的外部验收价值；
- **差异记录**：已执行测试与所引用的官方或 OpenRouter 声明不一致。

普通 probe 不要求每次写报告。没有独立价值的探测结果可由 Provider 页或当前状态保留简短入口；不得把一次 `accepted` 写成长期 capability 保证。差异记录必须保留来源声明、观察差异、endpoint、model ID、payload、账户/地域/网络边界和“不证明什么”。

## 真实 Provider 与客户端记录

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-09-09 | [OpenAI SDK Responses loopback](2026-09-09-openai-responses-sdk-loopback.md) | 固定官方 SDK 的 JSON/SSE 工具续轮、独立 wire oracle 与负向控制；无真实 Provider |
| 2026-09-09 | [DeepSeek Vision tool choice](2026-09-09-deepseek-vision-tool-choice.md) | auto/none 对照与 required/named 拒绝的双协议 JSON/SSE 复测；Target 收窄依据 |
| 2026-09-02 | [双协议能力探测记录](2026-09-02-dual-protocol-capability-matrix.md) | DeepSeek V4 Flash Vision、MiMo-V2.5、GLM-5.3-Flash、Qwen3.8-Max 的双协议 × 双交付固定 case；包含与注册声明的差异 |
| 2026-08-29 | [Bailian DeepSeek V4 Pro Responses 接入验证](2026-08-29-bailian-deepseek-v4-pro-responses.md) | 官方北京 Responses 声明、Target 注册修复、管理员 JSON/SSE probe 与本地下游 OpenAI SDK 请求 |
| 2026-08-29 | [OpenBridge Qwen3.7 Embeddings 与 Hindsight 兼容性验证](2026-08-29-openbridge-qwen37-embeddings-hindsight-compatibility.md) | 模型发现、float/维度、20/21 batch、归一化/稳定性、中英语义小样本，以及 Hindsight SDK Base64/user 阻断与本地修复边界 |
| 2026-08-27 | [Bailian Responses 三模型兼容性对比](2026-08-27-bailian-responses-model-comparison.md) | 北京 Models API 可见性及 GLM-5.2、DeepSeek V4 Flash 0731、Qwen3.8 Max 的 JSON/SSE、reasoning、structured output、工具续轮、state 与协议归因 |
| 2026-08-27 | [OpenRouter GLM-5.3-Flash 接入验证](2026-08-27-openrouter-glm-5-3-flash-integration.md) | Chat/Responses、图片、工具、structured output、Hermes `obc`/`obr` 与能力收窄 |
| 2026-08-10 | [OpenRouter Gemma strict schema 差异](2026-08-10-openrouter-gemma-strict-schema-mismatch.md) | OpenRouter structured-output 可见性与 strict JSON Schema 实测结果不一致 |

## 静态代码与配置审计

此类记录只证明指定 checkout 的源码注册、脱敏 configuration availability 和确定性合同测试，不替代真实 Provider 网络请求。

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-09-02 | [Generation IR 协议转换语义覆盖静态审计](2026-09-02-generation-ir-semantic-coverage-audit.md) | Generation Static/Event IR 转换路径、语义映射挂钩点、stateless Responses 语义覆盖与缺口 |
| 2026-08-25 | [全模型接入静态审计](2026-08-25-model-integration-static-audit.md) | Canonical、Target、Public Model、配置可用性、协议 surface 与证据缺口 |

## 维护规则

- 文件名以实际验证日期开头；已经发布的记录保留历史事实，不改写成当前状态，也不使用“最新”一词。
- 不保存 credential、账户标识、完整请求/响应、reasoning 正文、Provider request ID 或敏感业务内容。
- 模型信息可由 official website 或 OpenRouter 直接取得时，不在 evidence 复制完整 metadata。目录差异或未经请求验证的推论不构成测试差异；分别标注来源即可。
- 后续实现变化只更新当前实现、状态边界或对应 Provider 页；需要复测时新增一份带日期记录并由对应 owner 链接。
- 没有明确执行记录的 SDK、Agent、fallback、负载、长期运行或生产层必须写为未验证。
