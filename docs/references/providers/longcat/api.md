# LongCat API 与 reasoning 调研（复核于 2026-08-08）

## 来源与范围

- [API Quick Start](https://longcat.chat/platform/docs/)
- [Chat Completions](https://longcat.chat/platform/docs/api/chat.html)
- [Codex 接入](https://longcat.chat/platform/docs/Codex.html)
- [CC Switch 接入](https://longcat.chat/platform/docs/cc-switch)

本文只记录 LongCat 官方入口和 reasoning wire，不记录 OpenBridge 实现状态或私有账号结果。

## 观察事实

- OpenAI-compatible Chat 为 `POST https://api.longcat.chat/openai/v1/chat/completions`，使用 Bearer API key。
- Chat 的 `thinking` 对象是二态开关：`{"type":"enabled"}` 开启，`{"type":"disabled"}` 关闭。官方 Chat 参数表没有
  `low`、`medium`、`xhigh` 或 `max` 离散强度。
- 官方 Codex 与 CC Switch 配置都使用 `base_url=https://api.longcat.chat/openai/v1`、`wire_api=responses` 和
  `model_reasoning_effort="high"`；CC Switch 页面还明确说明 LongCat 原生支持 Responses，不需要本地协议路由。
- 官方资料因此直接支持“关闭/开启”二态模型行为，并给出 `high` 作为 Responses 的启用值；没有资料支持把 token budget
  或其他名字外推成更多离散 effort。

## 证据边界

官方文档没有列出 LongCat Responses 的完整 request schema 或全部 reasoning effort 枚举。本文不据此声称
`low`、`medium`、`xhigh`、`max` 可用，也不证明任一真实 API key、JSON/SSE、Bridge、负载或长期运行行为。
