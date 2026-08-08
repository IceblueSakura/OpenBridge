# LiteLLM 与 OpenRouter 模型信息综合调研

## 状态与前置文档

本综合文档建立在两个独立项目调研之上：

- [LiteLLM 模型信息接口与能力字段](../litellm/litellm-model-information-analysis.md)
- [OpenRouter API 与模型能力调研](../providers/openrouter/api.md)

本文只比较外部模型信息形状，不记录任何具体网关的模型注册、API 状态或实现设计。

## 1. 核心差异

| 维度         | LiteLLM                                         | OpenRouter                                        |
|--------------|-------------------------------------------------|---------------------------------------------------|
| 最小兼容列表 | `/models` 返回 OpenAI-compatible model id       | Models API 本身返回较完整目录对象                 |
| 详细资源     | deployment、model group 与全局 catalog 分开     | canonical model 与 endpoint detail 分开           |
| 单模型详情   | `/model/info` 偏 deployment；catalog 有独立详情 | `/api/v1/model/{author}/{slug}` 返回 `Model`      |
| 模态         | 多个 `supports_*` 旗标                          | `architecture.input_modalities/output_modalities` |
| 参数         | `supports_*` 与 supported OpenAI params         | `supported_parameters` + defaults                 |
| 上下文       | `max_input_tokens` / `max_output_tokens`        | `context_length` + top-provider output limit      |
| reasoning    | `supports_reasoning` 及目录扩展                 | 参数支持与可选 effort/default 元数据              |
| 供应信息     | deployment、model group、team、rate limit       | top provider、endpoint resource、user catalog     |
| 经济/质量    | price catalog、region 等                        | pricing、benchmarks、quality indexes              |

LiteLLM 的优势是明确展示 Proxy deployment 和丰富的全局目录字段；OpenRouter 的优势是用同一种 `Model` 对象组织
identity、architecture、context 和 supported parameters。

## 2. 可共同归纳的信息层

两个项目都说明“模型信息”至少包含不同层次：

1. **模型身份与生命周期**：id、名称、描述、创建/停用信息；
2. **目录能力**：任务类型、模态、上下文、参数、tools、structured output、reasoning；
3. **部署或供应信息**：Provider endpoint、deployment、rate limit、region、健康和账户可见性；
4. **经济与质量信息**：价格、benchmark 与派生指数；
5. **运行时观测**：latency、TTFT、error 和 throughput。

这些层次不能压缩成一个无来源的 capability 对象。尤其不能把某个 deployment 或 top provider 的能力自动解释为所有候选都保证的共同能力。

## 3. 未知语义

两套目录都有大量可选字段。跨项目无法统一推导“字段缺失 = 不支持”：

- LiteLLM 的部分 `supports_*` 字段来自不完整目录记录；
- OpenRouter 的可选 reasoning/limit 对象可能只在部分模型出现；
- Provider endpoint 或 deployment 可能比目录模型更窄；
- 目录也可能晚于 Provider 产品变化。

因此比较时应分别记录 `unknown`、明确不支持和明确支持，而不是把缺失值强制转成 false。

## 4. 聚合与安全边界

- LiteLLM model group 聚合多个 deployment；OpenRouter `top_provider` 是供应摘要。两者都不证明所有候选具有相同能力。
- deployment id、base URL、credential、team/access group 和账户过滤属于部署或控制面。
- pricing、benchmark、健康、TTFT 和错误率不等于协议能力。
- 外部目录可以作为研究证据，但不能单独证明某个实际 endpoint 当前接受对应请求。

## 5. 复核条件

LiteLLM routes、Model Catalog 字段和 OpenRouter Models API 都会演进。引用某个字段做兼容结论前，需要重新固定源码或官方响应快照，并区分
canonical model、deployment、Provider endpoint 与用户视图。
