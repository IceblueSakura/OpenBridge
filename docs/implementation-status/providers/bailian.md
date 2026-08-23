# Alibaba Cloud Model Studio Provider 状态

## 当前实现

- Provider family 为 `bailian`，OpenAI 兼容 origin 为 `https://dashscope.aliyuncs.com/compatible-mode/v1`，使用
  `bailian-primary` API-key pool。
- Qwen3.7 Plus/Max 与 Qwen3.8 Max 提供 Chat/Responses Native；Qwen3.6 27B 与 GLM-5.2 提供 Chat Native 和
  Responses-via-Chat Bridge；Qwen3.7 Text Embedding 提供独立 Embeddings Native。
- DeepSeek V4 Pro/Flash Target 是对应多 source Public Model 的 Chat 后备。
- Qwen Image 3.0/Pro 现在通过独立 `bailian-native` instance（`https://dashscope.aliyuncs.com/api/v1`）绑定
  DashScope 原生图像生成端点，并作为 Images Public Model 公开（`qwen-image-3.0`、`qwen-image-3.0-pro`）；下游以
  OpenAI Images 标准字段为主合同，并通过 typed `dashscope_extensions` 接受 prompt extension、negative prompt、seed、watermark；
  LiveTranslate 仍只有固定 Target，Qwen Audio ASR 仍只有 canonical Model，没有 executable Target。
- Qwen3.7/3.8 的七档 reasoning、Qwen3.6 的 `none/high`、GLM 的 `none/high/xhigh` 与 DeepSeek 各自固定档位按模型级
  contract 公开；只有已确认的 Chat API 在 `none` 需要时转换为 `enable_thinking:false`。
- Structured output 按 Target 收窄：Qwen3.7 Plus Chat 支持 strict JSON Schema，Responses 只保留 `json_object`；Qwen3.6
  Chat、Bailian DeepSeek Chat 只保留 `json_object`；其他 Target 不从 Provider ceiling 自动继承。
- GLM 与 Bailian DeepSeek Flash Chat 接受 `parallel_tool_calls`；其他 generation Target 保持关闭。
- Generation Public Model 安全接受唯一 Responses `include` 兼容提示 `reasoning.encrypted_content`；Bailian Native Responses 与
  Responses-via-Chat candidate 均不原生转发该值，由公共 planning 在各自 egress body 构造时删除。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/bailian/`](../../../src/providers/bailian/)。
- `tests/provider_contract.rs` 保护 Qwen/DeepSeek reasoning wire、operation surface 和 API-key 边界。
- `tests/forwarding_contract.rs` 保护 Public Model 投影、Native/Bridge、parallel、structured output 与 zero-egress 拒绝。
- `tests/embedding_forwarding_contract.rs` 保护 Qwen Embeddings 维度、输入、预算和 Bailian 成功体投影。
- `tests/images_forwarding_contract.rs` 保护 Images 严格目录、OpenAI→DashScope wire 转换、数量校验与零 egress。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)覆盖当时可见的
Qwen/GLM 与 DeepSeek 多 source Public Model 正常路径；[2026-08-10 Qwen3.6 矩阵](../evidence/real-provider/2026-08-10-qwen36-none-high-matrix.md)
记录该模型接入后的 Chat/Responses × JSON/SSE × `none/high`。

定向真实请求还确认 Qwen3.7 Text Embedding 的默认/显式允许维度、Qwen3.8 七档 reasoning、Bailian DeepSeek
`json_object`、GLM/DeepSeek Flash parallel 参数，以及上述 structured-output 收窄。Qwen3.7 Plus Responses 和 Qwen3.6 Chat
对 JSON Schema 的成功响应会静默降级，因此未公开为 strict 支持。

2026-08-22 本机 loopback 网关直连真实 DashScope 验证 `qwen-image-3.0`：T2I n=1/n=2 返回 `{created, data: [{url}]}`
（24h 签名 URL）；未知字段、`b64_json`、`size=4096x4096`、`n=7`、未知模型分别返回 400/404 且零 egress。见
[Images 功能状态](../features/native-images-generation.md)。

## 未证明边界

LiveTranslate 没有下游 executable interface；不能从 Target 或 Models 可见性推断可调用。Images 的 I2I、异步轮询、
`b64_json`、stream 未实现；图像内容质量与计费语义未验证。多模态、更多工具组合、强制 DeepSeek fallback、
其他账号/区域、外部 SDK/Agent、负载和长期运行未证明。

## 相关文档

- [Bailian API 参考](../../references/providers/bailian/api.md)
- [Bailian Models 参考](../../references/providers/bailian/models.md)
- [Embeddings 功能状态](../features/embeddings.md)
