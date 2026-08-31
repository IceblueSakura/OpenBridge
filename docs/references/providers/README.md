# Provider 调研索引

本目录以扁平文件保存外部 Provider 的协议、认证、endpoint、wire 和专项媒体调研。叶文档拥有来源、日期和证据边界；本索引只负责导航，不表示重新请求过任何 Provider。

## Provider 文档

| Provider | 协议与专项资料 |
|---|---|
| Alibaba Cloud Model Studio | [API](bailian-api.md) |
| DeepSeek | [API](deepseek-api.md) |
| Kimi | [API](kimi-api.md) |
| LongCat | [API 与 reasoning wire](longcat-api.md) |
| NVIDIA API Catalog / NIM | [API](nvidia-api.md) |
| OpenRouter | [API 与路由语义](openrouter-api.md) |
| Xiaomi MiMo | [API](xiaomi-api.md)、[图片 wire](xiaomi-image.md)、[音频 wire](xiaomi-audio.md) |
| Zhipu AI China / Z.AI | [API](zhipu-api.md) |

OpenBridge 当前 Model、Provider Target 与 Public Model 的关系由[实施状态映射](../../implementation-status/model-provider-mapping.md)唯一维护。

## 内容边界

- 本目录不保存 Provider 全量 Models 响应、模型能力表、context、modalities、tokenizer、reasoning levels、supported parameters、价格或原始模型元数据快照。
- official website 或 OpenRouter 已公开的模型信息优先用来源 URL、来源身份、复核日期和触发条件标注，不在本地重新展开。
- 模型能力以 `src/models/`、`src/providers/`、运行中的扩展 Models API 和 Provider 官方文档为准。
- Provider reference 只保留协议、认证、endpoint、request/response wire、错误和独立专项观察；模型名称只在解释映射或具体 wire evidence 时出现。
- 外部动态事实形成实现结论前必须重新核验；一次请求不证明其他账号、区域、模型、参数组合、负载或长期可用性。只有已执行测试与引用来源不一致时，才转入 implementation evidence 记录差异；official 与 OpenRouter 之间的静态目录差异不单独保存为测试结论。
