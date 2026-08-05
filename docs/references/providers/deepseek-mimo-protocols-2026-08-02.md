# DeepSeek 与 Xiaomi MiMo 协议入口综合对照（2026-08-02）

## 项目级前置文档

- [DeepSeek 协议入口快照](deepseek-protocol-2026-08-02.md)
- [Xiaomi MiMo 协议入口快照](xiaomi-mimo-protocol-2026-08-02.md)

本文只比较两个 Provider 的官方协议入口，不记录任何本地 Provider 注册、Route、capability 或验证状态。

## 协议对照

| 维度         | DeepSeek                               | Xiaomi MiMo                                           |
|--------------|----------------------------------------|-------------------------------------------------------|
| 官方基础地址 | `https://api.deepseek.com`             | `https://api.xiaomimimo.com/v1`                       |
| Chat         | `/chat/completions`                    | `/chat/completions`                                   |
| Responses    | 本次资料未确认                         | `/responses`                                          |
| 认证         | Bearer API key                         | `api-key` 或 Bearer                                   |
| 文档明确限制 | 本次未建立 Responses 契约              | Responses 不支持 `background`、`previous_response_id` |
| 文本模型快照 | `deepseek-v4-pro`、`deepseek-v4-flash` | `mimo-v2.5-pro`、`mimo-v2.5`                          |

## 综合结论

两家都提供 OpenAI-compatible Chat 入口，但不能仅凭 path 相同推断全部字段、错误、SSE、tool 或 reasoning 语义相同。MiMo 额外公开
Responses endpoint，并明确排除两项 state/background 字段；DeepSeek 在本次证据中只建立 Chat 入口。

认证 header 选择、模型可见性和完整 capability 仍需以各 Provider 的当前官方文档与实际账户验证分别确认。
