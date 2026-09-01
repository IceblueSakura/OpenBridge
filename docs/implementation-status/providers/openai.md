# OpenAI 接入进度与边界

注册与能力事实见 `src/providers/openai/`；当前接线见[映射](../model-provider-mapping.md)（当前仅有已注册、未发布 Public Model 的 Target）。

## 当前边界

- 当前没有成功的真实账号/Provider 验证；Models、Chat/Responses、Embeddings、图片、strict/parallel tool、structured output、state、
  配额、负载和长期运行均不能由静态 ceiling 推断。
- 四个已注册 Target（`openai-main`、`openai-gpt-5-5`、`openai-gpt-5-6-luna`、`openai-gpt-5-6-terra`）尚无下游 Public Model 引用；
  `gpt-5.6-sol` 的多 source 后备由 ChatGPT source 优先。

## 验证与证据

- 无带日期的真实 Provider 记录；静态 ceiling 与注册关系见代码。

## 代码 owner

`src/providers/openai/`。
