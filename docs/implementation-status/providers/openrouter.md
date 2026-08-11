# OpenRouter Provider 状态

## 当前实现

- Provider family 为 `openrouter`，固定 origin 为 `https://openrouter.ai/api/v1`，使用 `openrouter-primary` API-key pool。
- 固定 Target 分别绑定 DeepSeek V4 Flash、MiniMax M3 与 Gemma 4 31B；三个 Target 都提供 stateless Chat/Responses Native。
- DeepSeek V4 Flash 是对应多 source Public Model 的后备；MiniMax M3 是首选 source，NVIDIA Chat 为后备；Gemma 只使用
  OpenRouter，upstream model 固定为 `google/gemma-4-31b-it:free`。
- DeepSeek 保留 `json_object`；MiniMax 不公开 structured output；Gemma 只公开 `json_object`，关闭 strict function schema。
- Gemma 当前保留文本、JPEG/PNG 图片、function tool、parallel 参数和 JSON object；reasoning 未确认。
- Target 明确关闭 storage、continuation 和 background；客户端不能提交 OpenRouter attribution/routing header。
- Canonical alignment 只接受完整 ID 精确匹配：当前三个 Target 分别绑定
  `deepseek/deepseek-v4-flash`、`minimax/minimax-m3` 和 `google/gemma-4-31b-it`；不用同系列名称或相似 slug
  补齐本地模型事实。
- adapter 不注入 OpenRouter `provider.require_parameters:true`。OpenRouter 默认可将请求发送给未声明全部
  所传参数的 endpoint；因此本地 preflight 和 exact egress 只证明网关接受并传递参数，不证明
  OpenRouter 实际选中的 endpoint 应用了该参数。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/openrouter/`](../../../src/providers/openrouter/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护路径、模型、认证、data-only Responses terminal 和安全 header。
- `tests/forwarding_contract.rs` 保护三个 Public Model 的 Native 转发、能力预检和 multi-source 行为。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)中的
`minimax-m3` 正常首选路径使用 OpenRouter，并完成 Chat/Responses × JSON/SSE × `none/high`。该矩阵中
`deepseek-v4-flash` 首选 DeepSeek，不能证明 OpenRouter 后备。

2026-08-10 的 Gemma 定向请求支持当前文本、PNG 图片、parallel 参数与 `json_object` 收窄；strict JSON Schema 返回
markdown 包裹内容，未作为可靠 strict 能力公开。外部目录与协议快照见 [OpenRouter API 参考](../../references/providers/openrouter/api.md)
和 [模型目录](../../references/providers/openrouter/models.md)。

## 未证明边界

强制 DeepSeek fallback、远程图片、JPEG 实体内容、Gemma reasoning、MiniMax/NVIDIA failover、外部 SDK/Agent、Provider routing
偏好、负载和长期运行未证明。公开目录字段不自动成为当前 Target 的 executable capability。
