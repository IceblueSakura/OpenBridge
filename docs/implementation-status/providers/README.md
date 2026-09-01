# Provider 接入进度与边界

每个 Provider family 一页，镜像 `src/providers/<family>/` 目录。页面只记录接入进度、验证状态与该 Provider
的未证明边界；能力事实由注册代码与运行中的扩展 Models API 拥有，当前接线由
[Model 与 Provider 映射](../model-provider-mapping.md)唯一维护，外部协议事实由[参考资料](../../references/README.md)拥有。

| 页面 | Provider family |
|---|---|
| [bailian.md](bailian.md) | Alibaba Cloud Model Studio |
| [chatgpt.md](chatgpt.md) | ChatGPT |
| [deepseek.md](deepseek.md) | DeepSeek |
| [kimi_cn.md](kimi_cn.md) | Kimi CN |
| [longcat.md](longcat.md) | LongCat |
| [mimo.md](mimo.md) | Xiaomi MiMo |
| [nvidia.md](nvidia.md) | NVIDIA |
| [openai.md](openai.md) | OpenAI |
| [openrouter.md](openrouter.md) | OpenRouter |
| [zhipu_cn.md](zhipu_cn.md) | Zhipu AI China |

维护规则：

- 新 Provider 接入时同步新建一页；接入验证按 [evidence 规则](../evidence/README.md)新增带日期记录。
- 收窄或放宽注册能力时，在注册代码处注释引用日期化 evidence，并在本页更新边界。
- 本页不复制能力表、模型元数据、候选顺序或探测结果正文。
