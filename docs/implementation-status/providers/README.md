# Provider 接入进度与边界

每个 Provider family 一页，记录特有接线、协议例外、验证入口和未证明边界。能力事实由注册代码与运行中的扩展 Models API 拥有，当前
Public Model 接线由[Model 与 Provider 映射](../model-provider-mapping.md)唯一维护，外部协议事实由[参考资料](../../references/README.md)拥有。

| 页面 | Provider family |
|---|---|
| [bailian.md](bailian.md) | Alibaba Cloud Model Studio |
| [chatgpt.md](chatgpt.md) | ChatGPT |
| [deepseek.md](deepseek.md) | DeepSeek |
| [grok.md](grok.md) | Grok（xAI 订阅） |
| [kimi_cn.md](kimi_cn.md) | Kimi CN |
| [longcat.md](longcat.md) | LongCat |
| [mimo.md](mimo.md) | Xiaomi MiMo |
| [nvidia.md](nvidia.md) | NVIDIA |
| [openai.md](openai.md) | OpenAI |
| [openrouter.md](openrouter.md) | OpenRouter |
| [zhipu_cn.md](zhipu_cn.md) | Zhipu AI China |

## 维护规则

- Provider 页只保留该 family 的特有接线、例外和未证明边界；公共证据层（SDK/Agent、负载、长期运行、生产验收等分层限制）、永久非目标和完整能力表由对应 owner 拥有，不在每页逐层重复。
- 映射关系只改 [model-provider-mapping.md](../model-provider-mapping.md)，不要在 Provider 页复制 Public Model、候选顺序或全量模型清单。
- evidence 的准入条件、分层规则与维护规则由[evidence README](../evidence/README.md)唯一维护，本页不重复；Provider 页只保留与当前接入相关的证据指针。
- 注册代码发生收窄/放宽时，在代码注释和当前 Provider 页保留日期化 evidence 指针。
