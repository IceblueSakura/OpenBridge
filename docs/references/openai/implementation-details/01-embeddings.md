# Embeddings 协议实现细节

**目标状态：** 已批准的现阶段目标，也是当前唯一开发焦点；本文不表示代码已经实现。

## 范围与 wire contract

目标入口是受保护的 `POST /v1/embeddings`。当前官方 endpoint 使用 `application/json` 请求和 JSON 成功响应，不使用 SSE。核心请求字段是 `model`、`input`、`encoding_format`、`dimensions` 和可选 `user`。

`input` 是 tagged union，而不是单一字符串：

- 一个非空字符串；
- 多个字符串组成的批量输入；
- 一个 token integer array；
- 多个 token array 组成的批量输入。

成功响应必须保持 `object: "list"`、`data[]`、每项的 `object`、`embedding`、`index`、响应 `model` 和 `usage`。`encoding_format` 为 `float` 时向量是数值数组，为 `base64` 时保持上游编码；网关不得重新量化、舍入、截断、补零或自行重新编码。

官方当前说明 `dimensions` 只适用于部分较新的 embedding model。输入 token 上限、批量项数和单请求总 token 等数值属于具体 OpenAI profile，不应硬编码为所有 Provider 的共同能力。参考：[Create embeddings](https://developers.openai.com/api/reference/resources/embeddings/methods/create) 与 [Embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)。

## 公共契约与 registry

Embedding model 不应伪装成 `ModelMode::Chat`，也不应通过 Chat/Responses interface 暴露。registry 至少需要独立表达：

| 事实 | 最小含义 |
|---|---|
| operation | 该 Upstream API 原生提供 Embeddings |
| public model | 下游稳定 ID 与上游真实 model 分离 |
| input forms | string、string batch、token array、token-array batch 的允许集合 |
| output encoding | `float`、`base64` 的允许集合 |
| dimensions | 是否支持，以及可验证的范围/集合 |
| limits | 每输入、每批次和总 token/字节限制的 Provider-specific 上界 |
| vector identity | model family/version、维度、归一化和 tokenizer 契约 |

标准 `GET /v1/models` 仍可列出 Embedding Public Model，但扩展 Models DTO 必须让客户端区分 `embeddings` interface 与 Chat/Responses，且 `supported_parameters` 必须直接对应该接口的预检规则。

## 请求处理与 Native 转发

建议的可观察处理链是：

1. 认证、唯一 JSON `Content-Type` 和有界 body 校验；
2. 解析对象并读取下游 Public Model，不接受任意 upstream model、URL、header 或 credential；
3. 按 Embeddings 固定接口校验 `input` union、空值、批量、`encoding_format` 与 `dimensions`；
4. 生成只含受信 endpoint、认证和真实 upstream model 的 candidate request；
5. 保留其他被目标 Native profile 明确允许的 OpenAI 字段；
6. 要求成功响应为兼容 JSON，并有界校验 `data[].index`、向量编码和 usage 形状；
7. 原样返回安全 status/body/header，不把向量内容写入日志或 metrics。

首个版本不需要为向量建立内部 IR，也不应复用 Chat ↔ Responses Bridge。adapter 只改写 `model`，但仍需要 endpoint-specific parser 和 response contract，避免通用 JSON 路径接受生成字段或错误响应形状。

## 重试、fallback 与一致性

请求在响应提交前通常可重放；同一 Upstream Target 内的 credential 轮换仍须服从 attempt budget、timeout 和 body 大小限制。跨 Target/Provider fallback 默认关闭，除非 registry 显式证明以下身份全部等价：

```text
model family and immutable version
+ tokenizer/input encoding
+ output dimensions
+ normalization and distance contract
+ output encoding semantics
```

仅有相同模型显示名、相同维度或“OpenAI-compatible”标签不足以证明等价。若将不同 embedding 空间混入同一索引，HTTP 请求可能成功但检索语义已经损坏，因此必须 fail closed。

## 错误、安全与观测

- 非法 union、空输入、未知 encoding、未声明 dimensions 和超限在 egress 前返回稳定 OpenAI-compatible error。
- 上游错误可以保留安全 status、request id 和 allowlist rate-limit header，但不能带回 endpoint、credential 或原始敏感诊断。
- 不记录原始文本、token array、向量或 base64；指标只使用 Provider、operation、Public Model、status class 等低基数维度。
- `user` 是业务数据。除非有明确的下游 identity policy，否则不要用内部 user id、API key fingerprint 或 credential locator 自动填充。
- JSON body limit 与向量响应 limit 应分别配置；响应数组必须在解析和透传两条路径上有界。

## TDD 与验收矩阵

进入实现时应先建立失败测试：

| 层 | 必须证明的行为 |
|---|---|
| registry | Embedding Public Model 可独立注册；Chat model 不能误供给 Embeddings；capability elevation 启动失败 |
| ingress | 认证、Content-Type、JSON、body limit、缺失/未知 model 在出站前失败 |
| request | 四种 `input` 形状、`float`/`base64`、dimensions gate、model rewrite 和未知合法字段保留 |
| response | data 顺序/index、两种 embedding 编码、model/usage、超限和错误 JSON 处理 |
| resilience | 429/5xx/timeout 的有限 attempt；无等价声明时不跨向量空间 fallback；取消停止重试 |
| public API | OpenAPI 与扩展 Models interface 可被独立 Python/OpenAI SDK client 使用 |

确定性测试只证明本地 registry、routing 和 wire 行为。真实模型可用性、向量维度、归一化、输入限制、费用和跨服务等价性必须由经批准的 Provider 测试单独证明。

## 首个焦点的明确非目标

- 不在同一焦点实现 Chat/Responses 多模态、Images、Files、Vector Stores 或 Batch；
- 不做跨 Provider 向量转换、降维、归一化、缓存、索引或检索；
- 不实现 streaming、异步 job 或 embedding Bridge；
- 不把当前 OpenAI 限制值提升为所有 Provider 的全局默认；
- 不以真实 Provider 调用替代 deterministic contract tests。
