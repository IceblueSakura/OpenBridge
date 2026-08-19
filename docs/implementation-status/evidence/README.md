# 验证证据目录

本目录保存已经执行、带日期且边界明确的外部验证记录。记录按发生时间固定事实，不承担“当前实现”或“当前 Provider
能力”所有权；这些结论应由 [`implementation-status/README.md`](../README.md) 及对应功能/Provider 状态页解释。

证据层必须分开表述：确定性 Rust/Python 测试、loopback 客户端、外部 SDK、目标 Agent、真实 Provider、负载和长期运行
互不替代。真实 Provider 记录只证明当时 checkout、账号、网络、固定 endpoint、模型和 payload。

## 真实 Provider 记录

| 日期 | 记录 | 覆盖范围 |
|---|---|---|
| 2026-08-09 | [文字 Generation `none/high` 矩阵](real-provider/2026-08-09-text-generation-none-high-matrix.md) | 16 个当时可见文字 Public Model 的 Chat/Responses × JSON/SSE |
| 2026-08-10 | [Qwen3.6 27B `none/high` 矩阵](real-provider/2026-08-10-qwen36-none-high-matrix.md) | `qwen3.6-27b` 接入后的 Chat/Responses × JSON/SSE |
| 2026-08-19 | [DeepSeek V4 Native 与参数组合探测](real-provider/2026-08-19-deepseek-v4-native-and-parameters.md) | Pro/Flash 直连 Chat/Responses、logprobs、strict JSON/tool 组合 |

## 维护规则

- 文件名以实际验证日期开头；已经发布的记录不改写成当前状态，也不使用“最新”一词。
- 不保存 credential、账户标识、完整请求/响应、reasoning 正文、Provider request ID 或敏感业务内容。
- 后续实现变化只更新功能/Provider 状态页；需要复测时新增一份带日期记录并由状态页链接。
- 没有明确执行记录的 SDK、Agent、fallback、负载、长期运行或生产层必须写为未验证。
