# 验证证据目录

本目录保存已经执行、带日期且边界明确的外部验证记录。记录按发生时间固定事实，不承担“当前实现”或“当前 Provider
能力”所有权；这些结论由[当前实现](../current-state.md)和[当前状态边界](../current-boundaries.md)解释。

证据层必须分开表述：确定性 Rust/Python 测试、loopback 客户端、外部 SDK、目标 Agent、真实 Provider、负载和长期运行
互不替代。真实 Provider 记录只证明当时 checkout、账号、网络、固定 endpoint、模型和 payload。

## 真实 Provider 记录

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-08-10 | [OpenRouter Gemma strict schema 差异](2026-08-10-openrouter-gemma-strict-schema-mismatch.md) | OpenRouter structured-output 可见性与 strict JSON Schema 实测结果不一致 |

## 静态代码与配置审计

此类记录只证明指定 checkout 的源码注册、脱敏 configuration availability 和确定性合同测试，不替代真实 Provider 网络请求。

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-08-25 | [全模型接入静态审计](2026-08-25-model-integration-static-audit.md) | Canonical、Target、Public Model、配置可用性、协议 surface 与证据缺口 |

## 维护规则

- 文件名以实际验证日期开头；已经发布的记录不改写成当前状态，也不使用“最新”一词。
- 不保存 credential、账户标识、完整请求/响应、reasoning 正文、Provider request ID 或敏感业务内容。
- 模型信息可由 official website 或 OpenRouter 直接取得时，不在 evidence 复制完整 metadata。只有执行测试与引用来源不一致时才记录差异，并明确来源 URL/声明、观察结果、endpoint、model ID、payload、账户/地域/网络边界和不证明范围。
- official 与 OpenRouter 之间的字段差异、目录缺失或未经请求验证的推论不构成测试差异；分别标注来源即可。
- 后续实现变化只更新当前实现或状态边界；需要复测时新增一份带日期记录并由对应 owner 链接。
- 没有明确执行记录的 SDK、Agent、fallback、负载、长期运行或生产层必须写为未验证。
