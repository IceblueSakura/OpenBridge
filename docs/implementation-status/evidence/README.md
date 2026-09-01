# 验证证据目录

本目录保存已经执行、带日期且边界明确的外部验证记录。记录按发生时间固定事实，不承担"当前实现"或"当前 Provider
能力"所有权；这些结论由[当前实现](../current-state.md)、[当前状态边界](../current-boundaries.md)和
[Provider 分页](../providers/README.md)解释。

证据层必须分开表述：确定性 Rust/Python 测试、loopback 客户端、外部 SDK、目标 Agent、真实 Provider、负载和长期运行
互不替代。真实 Provider 记录只证明当时 checkout、账号、网络、固定 endpoint、模型和 payload。

## 记录类型

- **接入验证记录**：新接线模型或 Provider 的探测与验证结果。每次真实接入工作产出对应记录，即使结果与官方声明一致。
- **差异记录**：仅当执行测试与所引用的官方或 OpenRouter 声明不一致时新增；记录必须保留来源声明、观察差异、
  endpoint、model ID、payload、账户/地域/网络边界和"不证明什么"。

## 真实 Provider 记录

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-08-29 | [Bailian DeepSeek V4 Pro Responses 接入验证](2026-08-29-bailian-deepseek-v4-pro-responses.md) | 官方北京 Responses 声明、Target 注册修复、管理员 JSON/SSE probe 与本地下游 OpenAI SDK 请求 |
| 2026-08-29 | [OpenBridge Qwen3.7 Embeddings 与 Hindsight 兼容性验证](2026-08-29-openbridge-qwen37-embeddings-hindsight-compatibility.md) | 模型发现、float/维度、20/21 batch、归一化/稳定性、中英语义小样本，以及 Hindsight SDK Base64/user 阻断与本地确定性修复边界 |
| 2026-08-27 | [Bailian Responses 三模型兼容性对比](2026-08-27-bailian-responses-model-comparison.md) | 北京 Models API GLM 可见性及 GLM-5.2、DeepSeek V4 Flash 0731、Qwen3.8 Max 的 JSON/SSE、reasoning、structured output、工具续轮、state 与协议归因 |
| 2026-08-27 | [OpenRouter GLM-5.3-Flash 接入验证](2026-08-27-openrouter-glm-5-3-flash-integration.md) | Chat/Responses、图片、工具、structured output、Hermes `obc`/`obr` 与能力收窄 |
| 2026-08-10 | [OpenRouter Gemma strict schema 差异](2026-08-10-openrouter-gemma-strict-schema-mismatch.md) | OpenRouter structured-output 可见性与 strict JSON Schema 实测结果不一致 |

## 静态代码与配置审计

此类记录只证明指定 checkout 的源码注册、脱敏 configuration availability 和确定性合同测试，不替代真实 Provider 网络请求。

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-08-25 | [全模型接入静态审计](2026-08-25-model-integration-static-audit.md) | Canonical、Target、Public Model、配置可用性、协议 surface 与证据缺口 |

## 维护规则

- 文件名以实际验证日期开头；已经发布的记录不改写成当前状态，也不使用"最新"一词。
- 不保存 credential、账户标识、完整请求/响应、reasoning 正文、Provider request ID 或敏感业务内容。
- 模型信息可由 official website 或 OpenRouter 直接取得时，不在 evidence 复制完整 metadata。official 与 OpenRouter 之间的
  字段差异、目录缺失或未经请求验证的推论不构成测试差异；分别标注来源即可。
- 注册代码中的能力收窄/放宽应在注释处引用日期化 evidence，而不是复述探测结论；evidence 承载"当时观察到什么"。
- 后续实现变化只更新当前实现、状态边界或对应 provider 页；需要复测时新增一份带日期记录并由对应 owner 链接。
- 没有明确执行记录的 SDK、Agent、fallback、负载、长期运行或生产层必须写为未验证。
