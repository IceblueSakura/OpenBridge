# Embeddings 能力需求

## 范围

本页只定义 OpenAI-compatible `POST /v1/embeddings` 的输入、输出、能力、资源和失败边界。它不定义图片、文件、音频或其他
Chat/Responses 媒体能力；共同的能力分层、固定 Route 与证据规则见
[媒体扩展共同规则](embedding-and-native-multimodal.md)。当前实现事实与实际验证见
[Embeddings 实施状态](../implementation-status/features/embeddings.md)。

## 1. 用户结果

已认证客户端应能使用稳定 Embedding Public Model 调用 `POST /v1/embeddings`，而无需知道上游 Provider、真实 model、endpoint 或
credential。接口必须：

- 接受 OpenAI-compatible JSON 中的 string、string array、token array 和 token-array array 输入；
- 拒绝空字符串、空集合、混合类型数组、非法 token 值和 profile 未声明的输入形状；
- 按所选固定接口能力校验 `encoding_format`、`dimensions`、批量、可直接计算的 token-array 数量和字节限制；
- 只把 Public Model 改写为 registry 中的真实 upstream model，保持该 Native profile 明确允许的其他字段；
- 将成功响应的 `model` 归一为下游 Public Model，并保持有序 `data[]`、每项 `object`/`index`/`embedding`、响应 `object` 与
  `usage`；
- 不改变向量数值、Base64 内容、维度、项目顺序或 index 语义；
- 在没有等价 vector identity 声明时禁止跨 Provider/模型 fallback；
- 对非法输入、不支持能力、响应形状错误和超限返回安全、稳定错误。

Embeddings 是独立 operation，不得伪装成 Chat/Responses 文本生成，也不通过 Bridge、文本占位或网关本地向量变换实现。

## 2. `interfaces.embeddings` 公共契约

一个可调用的 Embeddings interface 至少公开：

| 字段                   | 语义                                                                                                                                |
|------------------------|-------------------------------------------------------------------------------------------------------------------------------------|
| `input_forms`          | `string`、`string_array`、`token_array`、`token_array_array` 的非空保证集合                                                         |
| `encoding.default`     | 省略 `encoding_format` 时保证的 `float` 或 `base64` wire                                                                            |
| `encoding.allowed`     | `null` 或可显式请求的 `float`/`base64` 非空集合；不得由网关本地转换补足                                                             |
| `dimensions.default`   | 省略 `dimensions` 时保证返回的正整数维度                                                                                            |
| `dimensions.allowed`   | `null`、闭区间或离散集合；`null` 表示请求不得携带 `dimensions`                                                                      |
| `limits`               | 有效批量项数、单输入/总 token 上界，以及 `locally_counted_input_forms`；部署级 request、JSON response 与 replay budget 另行统一执行 |
| `supported_parameters` | 除必填 `model`/`input` 外可执行的顶层可选字段，例如 `encoding_format`、`dimensions`、`user`                                         |

第一版 `locally_counted_input_forms` 只包含 token-array 两种形状；string/string-array 的 token 上界是 Provider-enforced
contract，不能用字符或 UTF-8 字节估算冒充本地预检。

内部 vector identity 至少约束 immutable model/checkpoint、tokenizer/input encoding、默认与可选维度、归一化/距离语义及编码语义。
它只用于 registry 校验与 fallback 安全，不得向下游暴露 upstream model 或 Provider identity。

## 3. 编译、预检与响应预算

- input form、显式 encoding 和 dimension domain 取全部静态可执行 Route 的交集；default 必须一致。
- explicit encoding/dimension 交集为空但 default 一致时，把 `allowed` 设为 `null` 并移除相应 `supported_parameters`；default
  不一致时拒绝 registry 编译。
- `max_inputs` 必须用 checked arithmetic 被 gateway batch/JSON response budget、最大公开维度、允许 encoding 的最坏序列化上界和
  固定 envelope 收窄；无法证明至少一个输入的合法响应受限时启动失败。
- 请求分析冻结实际 input form、encoding、dimension、批量和可直接计算的 token/byte facts；通过 preflight 后不得按请求重新筛选
  Route。
- 成功体必须在下游提交前完成有界 JSON shape 校验；网关只投影 Public Model，不转换向量或编码。

## 4. 重放、取消与数据保护

- 请求 body 不超过 replay budget 且响应尚未提交时才可有限重放；超过 replay budget 但仍合法的请求只执行第一次 attempt。
- 只有 vector identity 等价得到显式 registry 证明时才允许跨 Target；当前目标不以同名模型推断等价。
- 下游取消必须停止发送、接收和待执行 backoff；任何成功 body byte 提交后不得 retry 或拼接第二个响应。
- 原始文本、token array、向量、Base64 与 `user` 不得进入日志、trace attribute 或 metrics label。
- Embeddings operation 固定使用低基数 `embeddings_create`；只记录明确返回的 input/total token，不虚构 output token 或生成速度。

## 5. 验收

| ID     | 应被保护的可观察行为                                                                                                                         |
|--------|----------------------------------------------------------------------------------------------------------------------------------------------|
| EMB-01 | interface 的 forms、encoding、dimension、limits 与参数列表来自同一执行接口，并与 `/v1/embeddings` preflight 一致。                           |
| EMB-02 | 四种输入、model 双向投影、float/base64、dimensions、data/index/object/usage 满足固定 contract，向量不被转换。                               |
| EMB-03 | 无 vector identity 等价证明时不发生跨 Provider/模型 fallback；retry、取消、响应预算和首输出 commit 可确定复现。                             |
| EMB-04 | 标准 Models 仍为四字段；扩展 Models 不暴露 Provider、Target、Route、upstream model、credential、vector identity 或运行状态。                |

## 6. 非目标与参考

非目标包括 embedding Bridge、向量归一化、降维、缓存、索引、检索和根据向量能力动态选路。

- [Embeddings 协议调研](../references/openai/protocol-details/01-embeddings.md)
- [OpenAI Embeddings 与多模态 API 关系](../references/openai/embedding-and-multimodal-forwarding.md)
