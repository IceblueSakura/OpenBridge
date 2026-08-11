# Provider 实施与实测状态

本目录按闭合 `ProviderKind` family 汇总当前 checkout 的固定注册、模型级收窄、确定性证据、真实 Provider 证据与未证明边界。
它不替代 [`features/`](../features/) 中按功能划分的状态页，也不复制 [`references/providers/`](../../references/providers/)
中的外部协议事实。

## 证据术语

- **真实 Provider 证据**：固定可信 endpoint 上实际请求获得满足判据的结果；只适用于记录中的账号、网络、模型和 payload。
- **端到端网关证据**：请求经过本地 OpenBridge 下游入口和正常首选 Route；没有故障注入时不证明后备 source。
- **确定性证据**：registry、planning、mock transport 或 fixture test 保护当前代码合同；不代表真实上游。
- **未证明**：没有相应证据，或某一层证据不足以外推到 SDK、Agent、fallback、负载、长期运行或生产环境。

HTTP 200、字段被接受或参数被静默忽略本身不构成语义能力。例如 function tool 必须返回结构有效的 tool call；structured
output 必须符合声明的模式，不能把静默降级计为 strict 支持。

## 当前 Provider family

| Provider family | 状态页 | 真实证据摘要 |
|---|---|---|
| ChatGPT | [ChatGPT](chatgpt.md) | 2026-08-09 五个含 ChatGPT source 的 Public Model 文本矩阵；其中四个为 ChatGPT-only |
| OpenAI | [OpenAI](openai.md) | 没有成功的当前真实 Provider 记录；`gpt-5.6-sol` 正常路径矩阵不能证明 OpenAI 后备 |
| LongCat | [LongCat](longcat.md) | 2026-08-09 文本 `none/high` 正常路径矩阵 |
| DeepSeek | [DeepSeek](deepseek.md) | 2026-08-11 直连 Chat/Responses structured-output 探测 |
| Xiaomi MiMo | [MiMo](mimo.md) | 文本、图片、音频和 function tool 的定向真实请求 |
| OpenRouter | [OpenRouter](openrouter.md) | MiniMax 正常路径矩阵与 Gemma 定向真实请求；未强制 DeepSeek 后备 |
| NVIDIA | [NVIDIA](nvidia.md) | Nemotron Embeddings 定向真实请求；MiniMax 后备未强制验收 |
| Alibaba Cloud Model Studio | [Bailian](bailian.md) | Qwen/GLM/DeepSeek/Embeddings 的定向真实请求与 Qwen3.6 矩阵 |
| Kimi CN | [Kimi CN](kimi-cn.md) | 2026-08-09 `kimi-k3` 正常路径矩阵与参数边界 |

## 维护边界

- 每个 `ProviderKind` 只有一个当前状态页；Target/Public Model 变化时原地更新，不把实施过程附加成时间线。
- 当前固定 Target、operation 与模型级能力从 live source 确认；运行时 Models 可见性仍受 active credential pool 收窄。
- 带日期的可复用外部验证写入 [`evidence/`](../evidence/README.md)，Provider 页只链接并解释其适用边界。
- 不记录 credential、账户、完整请求/响应、原始 Base64、Provider request ID 或敏感业务内容。
