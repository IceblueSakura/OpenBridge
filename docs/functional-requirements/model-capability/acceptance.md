## 1. 功能验收要求

| ID       | 应被保护的用户可观察行为                                                                                                                                               |
|----------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| MODEL-01 | 标准 list/retrieve 只返回四字段对象，且详情与列表元素相同。                                                                                                            |
| MODEL-02 | 扩展 list/retrieve 返回同一个固定能力对象；参数只由目标接口公开，且不包含部署、凭据、价格或运行状态。                                                                  |
| MODEL-03 | active/deprecated 模型可见；retired 或无可执行接口的模型不可见、不可调用。                                                                                             |
| MODEL-04 | 较弱首选 Route 与较强后续 Route 的交集仍拒绝能力请求，且不发生 egress。                                                                                                |
| MODEL-05 | 能力预检通过后保留全部配置 Route 的原顺序，不按请求能力跳过或重排。                                                                                                    |
| MODEL-06 | unknown 能力 fail closed；token 上限与集合按保守交集计算。                                                                                                             |
| MODEL-07 | Chat、Responses 与 Embeddings 能力相互隔离，不能用一个接口的能力扩大另一个接口。                                                                                       |
| MODEL-08 | 未知模型和 retired 模型统一返回安全 `model_not_found`；能力不足返回 `unsupported_model_capability`。                                                                   |
| MODEL-09 | registry 在启动时拒绝非法身份、生命周期、上下文、模态、引用和能力扩大。                                                                                                |
| MODEL-10 | Embeddings dimension domain、Chat/Responses source-aware 输入与 mode-aware 音频输出由 Models projection 和 preflight 共享，不能由 bool、Native passthrough 或请求期 Route 过滤扩大。 |
| MODEL-11 | `capabilities.tasks` 只由唯一 canonical task 按闭合映射产生；不同 task 的 Route 不能编译进同一 Public Model。                                                    |
| MODEL-12 | Provider 完整 audio ceiling、单个 executable profile 与 canonical task 在启动期逐层校验；VoiceClone conditioning 不进入 content-understanding input。             |
| MODEL-13 | Structured Output 的 Provider/Target profile、Public 交集、Models 投影与请求预检共享一个闭合联合；无共同 mode 时不公开幽灵支持或参数。             |
| MODEL-14 | generation reasoning `levels`、`accepted_levels` 与 `input_policy` 共享同一固定接口；正向归一化在 candidate 展开前执行一次，`none` 保持独立，标准 Models 投影不变。 |
| MODEL-15 | Responses `response_includes` 按具体 wire 值的 public accepted set 保守相交并直接供 preflight 使用；candidate forwarded set 保持私有；接受值不保证输出 item，唯一 approved omitted-equivalent include hint 可在 Native/Bridge candidate planning 中逐值删除；`prompt_cache_key` 作为全部 generation interface 接受的 best-effort 参数公开，candidate 按 concrete API 精确转发或删除，不产生独立缓存效果字段。 |
| MODEL-16 | 扩展 list 的 `native_protocol` 只命中含对应 Native candidate 的 Public Model；Bridge-only interface 被排除，省略参数保持完整列表，非法、重复或未知 query 显式失败且响应不泄漏拓扑。 |

## 2. 非目标

- 根据能力、质量、成本或 benchmark 自动选模；
- 按请求能力筛选、打分、加权或重排 Route；
- 在 Models API 中暴露 deployment、endpoint、credential、健康、价格、配额、指标或 benchmark；运行指标只通过独立 OTLP metrics
  signal 导出，不属于模型目录或模型能力契约；
- 从 LiteLLM、OpenRouter、Provider `/models` 或 probe 动态发现和注册模型；
- 模型推荐、自动迁移、alias resolution、ACL、分页搜索，或除 `native_protocol` 外的通用 capability query API；
- 在没有完整协议语义时，仅因模型本体声称支持就放行 hosted/custom tool、audio/file、state、embedding 参数或 opaque reasoning。
