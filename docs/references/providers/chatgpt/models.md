# ChatGPT Codex 上游模型目录快照（2026-08-10）

## 来源与采集边界

- 采集时间：2026-08-10，Asia/Shanghai。采集使用当时有效的 OAuth session；快照不保存 access/refresh token、账户标识或
  Authorization 值。
- 端点：`GET https://chatgpt.com/backend-api/codex/models?client_version=0.146.0`
- 请求 header：
  - `Authorization: Bearer <access_token>`
  - `originator: codex_cli_rs`
  - `User-Agent: codex_cli_rs/0.146.0 (Linux unknown; x86_64) unknown`
  - `accept: text/event-stream`
- 原始数据：[脱敏原始 JSON 快照](models-2026-08-10.json)（9 个模型，366 KB，含 service_tiers、truncation_policy、
  experimental_supported_tools、model_messages、base_instructions 等完整字段）。
- 快照会过期：模型集合、字段与 API 可用性会随 ChatGPT 后端变化，本文不是永久事实。

## 模型清单摘要

| slug | supported_in_api | 模态 | reasoning levels | context / max | default level | 备注 |
|---|---|---|---|---|---|---|
| `gpt-5.3-codex-spark` | ❌ False | text | low ~ xhigh | 128K / 128K | high | **上游 API 已关闭** |
| `gpt-5.4` | ✅ True | text+image | low ~ xhigh | 272K / 1M | medium | list visibility |
| `gpt-5.4-mini` | ✅ True | text+image | low ~ xhigh | 272K / 272K | medium | list visibility |
| `gpt-5.5` | ✅ True | text+image | low ~ xhigh | 272K / 272K | medium | list visibility |
| `gpt-5.6-luna` | ✅ True | text+image | low ~ max | 272K / 272K | medium | list visibility |
| `gpt-5.6-terra` | ✅ True | text+image | low ~ **ultra** | 272K / 272K | medium | list visibility |
| `gpt-5.6-sol` | ✅ True | text+image | low ~ **ultra** | 272K / 272K | low | list visibility |
| `gpt-5.6-sol-wm` | ❌ False | text+image | low ~ ultra | 272K / 272K | low | 隐藏（World Model 变体） |
| `codex-auto-review` | ✅ True | text+image | low ~ max | 272K / 272K | medium | hide visibility |

全部模型：`supports_parallel_tool_calls: True`、`supports_search_tool: True`、
`prefer_websockets: True`；`supports_reasoning_summary_parameter` 除
`gpt-5.3-codex-spark` 外均为 True。

## 观察与边界

- `supported_in_api:false` 是快照时的上游目录事实，只证明该 slug 当时不应作为可调用 API 模型；它不证明永久下线。
- `supported_reasoning_levels` 是结构化数组，且 `gpt-5.6-sol/terra` 当时包含 `ultra`；任何客户端或网关如何映射该值都需要
  独立契约与请求验证。
- `input_modalities:text+image`、`supports_reasoning_summary_parameter:true` 与其他目录标记只表示上游发布的模型元数据，不能单独
  证明图片输入、summary 请求参数、SSE 事件、工具执行、额度或生产可用性。
