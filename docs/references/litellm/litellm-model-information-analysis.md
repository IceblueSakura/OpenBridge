# LiteLLM 模型信息接口与能力字段调研

## 状态与证据

- 调研日期：2026-08-03
- 当前源码复核基线：`BerriAI/litellm` commit `23de7a15d9d40006ee596e617475ba101d60c5e9`
- 阅读范围：Proxy model routes、Model Catalog、模型价格/上下文目录和 router types
- 本文只记录 LiteLLM 的模型、deployment、model group 与 catalog 粒度。

## 1. 接口分层

| 接口 | 主要对象 | 返回重点 |
| --- | --- | --- |
| `/models`、`/v1/models` | OpenAI-compatible 可调用 model id | 最小 `Model` 条目，可受团队、路由和健康过滤 |
| `/models/{model_id}` | 单个兼容 model id | 单个最小模型对象 |
| `/model/info`、`/v1/model/info` | Proxy deployment | `model_name`、`litellm_params`、可扩展 `model_info` |
| `/v2/model/info` | DB-backed deployment | 分页、搜索、团队和 deployment 元数据 |
| `/model_group/info` | 逻辑模型组 | 多 deployment 的 Provider、限制、成本与 capability 聚合 |
| `/model_catalog` | 全局模型目录 | mode、上下文、价格、模态及大量 `supports_*` 字段 |
| `/model_catalog/{model_id}` | 单个目录模型 | 一条目录能力/成本记录 |
| model metrics endpoints | 运行时观测 | latency、TTFT、failure 和 streaming 统计 |

这些接口并不共享同一种“模型”语义：兼容列表、Proxy deployment、逻辑组、全局目录和运行时指标需要分开理解。

## 2. 三种主要模型粒度

### 2.1 兼容列表模型

`/models` 面向 OpenAI-compatible client，主要回答可以使用哪些 model id。它不承诺完整上下文、tool、reasoning 或模态能力。

### 2.2 Deployment 与 model group

`/model/info` 把调用名、上游参数和可扩展 `model_info` 放在一条 deployment 记录中。`model_group/info` 再把多个 deployment 聚合，可能同时返回 Provider、TPM/RPM、成本、访问控制和 `supports_*` 能力。

这种聚合说明管理面可以展示丰富信息，但一个 group 的字段不自动保证每个 deployment 都具有相同能力。

### 2.3 Model Catalog

独立 Model Catalog 更接近静态模型能力目录。可观察字段包括：

- identity、Provider/source、deprecation date；
- `mode`，覆盖 chat、embedding、image、audio、moderation、rerank 等任务；
- `max_input_tokens`、`max_output_tokens`；
- vision、audio、PDF、video 等模态旗标；
- function calling、parallel tools、response schema、system message；
- reasoning、prompt caching、supported endpoints；
- price、region、vector size 等经济或供应字段。

缺失的可选 `supports_*` 字段不能在没有目录语义保证时一律解释为 false。

## 3. 字段分类

```text
Model catalog entry
├── identity and lifecycle
├── task mode
├── input/output limits
├── modalities
├── tools and structured output
├── reasoning and cache hints
├── endpoint hints
└── economics, region and operations
```

最后一层与协议能力不同。deployment id、base URL、credential locator、team/access group、健康、成本和 runtime metrics 也不属于同一种静态模型事实。

## 4. 适用边界

- `/models` 是兼容列表，不是详细 capability schema。
- `/model/info` 是 deployment 资源，不能当作模型本体对象。
- model group 聚合可能混合多个 Provider/deployment，不能自动形成共同能力下界。
- Model Catalog 是全局目录声明，不证明某个本地 deployment 当前可执行。
- 路由、team、budget 和 virtual-key 字段是 LiteLLM Proxy 管理面行为。

## 一手资料

- [LiteLLM Proxy model management](https://docs.litellm.ai/docs/proxy/model_management)
- [LiteLLM Model Catalog API](https://api.litellm.ai/docs)
- [`proxy_server.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/proxy/proxy_server.py)
- [Model price and context catalog](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/model_prices_and_context_window.json)
- [`litellm/types/router.py`](https://github.com/BerriAI/litellm/blob/23de7a15d9d40006ee596e617475ba101d60c5e9/litellm/types/router.py)

