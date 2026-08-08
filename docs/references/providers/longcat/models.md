# LongCat 2.0 模型事实（复核于 2026-08-08）

## 来源与范围

- [LongCat Retrieve Model](https://longcat.chat/platform/docs/api/model)
- [OpenRouter `GET /api/v1/models`](https://openrouter.ai/api/v1/models)，精确匹配 `meituan/longcat-2.0`

## 观察事实

- LongCat 官方模型详情声明 `LongCat-2.0` 为 text-to-text，context length 为 1,048,576，`supported_parameters`
  包含 `thinking`。
- OpenRouter 精确记录声明 reasoning 非强制、默认开启并支持 token budget，但没有 `supported_efforts`。
- 官方 Chat 的 `thinking` 开关与 OpenRouter 的非强制 reasoning 相互印证二态行为；官方 Codex 配置另给出标准启用值
  `high`。没有来源支持其他离散 effort。

## 证据边界

模型详情和 OpenRouter 动态目录不是 Provider 可用性测试。token budget 是数量上限，不等价于离散 reasoning level；
本文不从同系列旧模型、Agent 客户端或 HTTP 200 静默接受外推能力。
