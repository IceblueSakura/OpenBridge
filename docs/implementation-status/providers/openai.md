# OpenAI 接入进度与边界

注册与能力事实见 `src/providers/openai/`；当前 Generation Targets 已注册但尚未发布 Public Model，也没有公开的 Embeddings Target；关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- Generation Targets 当前没有下游 Public Model 引用，不能将它们写成当前 fallback；OpenAI 直连当前不公开任何 Public Model，完整关系见映射。
- 没有成功的真实账号/Provider 验证；Models、Chat/Responses、图片、strict/parallel tool、structured output、state、配额、负载和长期运行都不能由静态 ceiling 推断。

## 验证与证据入口

- 无带日期的真实 Provider 记录；当前只保留代码注册和映射入口。

## 代码 owner

`src/providers/openai/`。
