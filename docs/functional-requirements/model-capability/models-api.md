## 1. 接口契约

| 接口                                | 成功响应                                        |
|-------------------------------------|-------------------------------------------------|
| `GET /v1/models`                    | `object: "list"` 与严格四字段 `StandardModel[]` |
| `GET /v1/models/{model}`            | 一个严格四字段 `StandardModel`                  |
| `GET /openbridge/v1/models`         | 可选 Native generation 协议筛选后的 `object: "list"` 与完整 `PublicModelInfo[]` |
| `GET /openbridge/v1/models/{model}` | 一个完整 `PublicModelInfo`                      |

## 2. 共同要求

- 四个接口使用与生成接口相同的静态 Bearer 认证。
- `StandardModel` 严格只有 `id`、`object: "model"`、`created` 和 `owned_by: "openbridge"`。
- 扩展 generation interface 的 `reasoning.levels` 是实际可执行交集，`accepted_levels` 是下游可提交的标准词汇，`input_policy`
  明确两者间的固定解析规则；三者不得泄漏 Route、Provider 或 wire mapping。
- 扩展 generation interface 的 `response_includes` 只包含全部固定候选共同接受且能安全处理的精确 wire 值，不构成输出 item 保证；`prompt_cache_key` 通过
  `supported_parameters` 表示下游接受，不表示每个 candidate exact-forward，也不得重新投影为"缓存受支持"或 cache-hit 保证。candidate forwarded/omitted 事实保持私有。
- 扩展 list 接受至多一个 `native_protocol=chat_completions|responses`。省略时返回完整可见目录；存在时只保留目标 downstream
  protocol 的固定 execution interface 至少包含一条 `Native` candidate 的 Public Model。仅有 `Bridged` candidate 的同协议
  interface 不得命中；筛选不得公开 candidate、Route 或部署事实，也不得改变模型顺序或请求 Route 顺序。
- 空值、未知值、重复 `native_protocol` 和其他未知 query parameter 必须返回 HTTP 400 `invalid_request_error`，并在 `param`
  中定位对应 query parameter；不得静默忽略并返回完整目录。
- 同一 snapshot 下，retrieve 必须与对应列表元素逐字段相同；列表按 Public Model id 确定性排序。
- 未知、retired 或当前不可用模型返回 HTTP 404、`model_not_found`，`param` 为 `model`，不得区分内部存在性。
- 固定接口契约不支持请求时返回 HTTP 400、`unsupported_model_capability`，并保证上游调用次数为零。
- 已识别但未纳入当前协议契约的能力可以返回独立稳定的 `unimplemented_request`，不得尝试透传猜测。
- 除上述单一 Native generation 协议筛选外，不提供分页、搜索、排序、模型 ACL、通用能力过滤或动态刷新。
