# Kimi K3 模型参数

## 来源与范围

- 官方 [模型参数参考](https://platform.kimi.com/docs/api/models-overview)，复核日期：2026-08-09；
- 官方 [Kimi K3 快速开始](https://platform.kimi.com/docs/guide/kimi-k3-quickstart)，复核日期：2026-08-09。

本页只保存与当前 Kimi K3 请求兼容直接相关的公开模型事实。

## 已确认事实

- `kimi-k3` 的 `temperature` 固定为 `1.0`、`top_p` 固定为 `0.95`、`n` 固定为 `1`，
  `presence_penalty` 与 `frequency_penalty` 固定为 `0`。
- 官方说明固定参数不能修改，传入其他值会报错，并建议调用 Kimi K3 时不要显式传入这些字段。
- Kimi K3 始终启用 reasoning；`reasoning_effort` 支持 `low`、`high`、`max`，默认 `max`。固定 sampling 参数与
  reasoning effort 是不同能力，不能通过删除 sampling 字段推导或改变 reasoning level。

## 证据边界

官方文档没有把所有 OpenAI-compatible 扩展参数逐项列为支持。未列字段仍需要按具体 endpoint 和模型做真实请求验证；文档本身不证明
OpenBridge 的 Bridge、streaming、fallback 或参数删除行为。
