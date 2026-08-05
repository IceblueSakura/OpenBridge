# Embeddings 协议实现细节

**当前边界：** 当前 checkout 已按本文首版契约提供 `/v1/embeddings`；已运行证据与尚未验证层以[当前实现说明](../../../implementation-status/current-implementation.md)为准。本文保留外部 wire 事实和实现接缝，不把 deterministic loopback 推导为真实 OpenAI Provider 验收。

## 1. 官方 wire contract

入口是受保护的 `POST /v1/embeddings`。当前官方 endpoint 使用 `application/json` 请求和 JSON 成功响应，不使用 SSE。请求对象只建模下列标准字段：

| 字段 | 形状 | 第一版网关语义 |
|---|---|---|
| `model` | non-empty string | 必填 Public Model；由受信 Route 改写为 upstream model |
| `input` | string、string[]、integer[]、integer[][] | 必填 tagged union；禁止空值、混合数组和形状歧义 |
| `encoding_format` | `float` 或 `base64` | 可选；只有固定 interface 声明时可用，不在网关转换 |
| `dimensions` | positive integer | 可选；必须属于接口公开的 allowed domain |
| `user` | string | 可选业务数据；只有 profile 允许时 Native 保留，禁止自动填充内部身份 |

`input` 的判别规则必须确定且一次完成：

| JSON 形状 | 归类 | 明确拒绝 |
|---|---|---|
| `"text"` | `string` | 空字符串 |
| `["a", "b"]` | `string_array` | 空数组、空成员、混合字符串/数字 |
| `[1, 2, 3]` | `token_array` | 空数组、非整数、负数或不能表示为 `u32` 的 token 值 |
| `[[1, 2], [3]]` | `token_array_array` | 空外层、空内层、混合层级、成员类型或 token 范围非法 |

成功响应必须具有 `object: "list"`、`data[]`、每项 `object: "embedding"`、`embedding`、唯一且与输入位置对应的 `index`、响应 `model` 和 `usage.prompt_tokens`/`usage.total_tokens`。`float` wire 的 embedding 是数值数组；`base64` wire 必须保持上游返回的字符串内容，网关不得重新量化、舍入、截断、补零或自行重新编码。

截至 2026-08-04，OpenAI 官方 reference/生成 SDK 类型说明每个输入最多 8192 tokens、一个输入数组最多 2048 项、单请求所有输入合计最多 300,000 tokens；`dimensions` 只适用于 `text-embedding-3` 及之后模型。官方 guide 给出的默认维度是 `text-embedding-3-small=1536`、`text-embedding-3-large=3072`。这些数值只属于当前 OpenAI profile，不能提升为所有 Provider 的默认值。

官方资料：[Create embeddings](https://developers.openai.com/api/reference/resources/embeddings/methods/create)、[Embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)、[官方 Python SDK request type](https://github.com/openai/openai-python/blob/main/src/openai/types/embedding_create_params.py)。

## 2. 独立 operation 与任务类型

Embeddings 不复用 `ApiProtocol` 中的 generation 语义或 `GenerationCapabilities`。当前实现独立表达：

- endpoint/operation identity：Embeddings Create；
- Canonical task：Embedding，而不是 `ModelMode::Chat`；
- Upstream API：受信 relative path、JSON transport、upstream model 和 Embeddings capability；
- Route：只允许 Embeddings → Embeddings Native；
- Public Model interface：`interfaces.embeddings`；
- request/response parser：不接受 stream、messages、tools、reasoning 或 generation output 字段。

Rust exhaustive match、registry 引用验证、Provider adapter、ingress/OpenAPI 和测试已同步更新；后续修改仍不得只在 Router 增加路径后把请求塞进 Chat/Responses pipeline。

该接口和扩展 schema 在实现前尚未发布，因此直接按首版最佳实践建模：`schema_version` 保持 `"1"`，不保留 generation 布尔占位、旧 DTO、兼容 enum variant、请求字段 alias、双读写或转换 shim。bootstrap 新预算字段同样必须显式提供；缺失时启动失败，不从旧的 request limit 推导默认值。

## 3. Embeddings capability contract

### 3.1 推荐的类型化字段

`interfaces.embeddings` 应与 Chat/Responses 的 generation interface 分型。以下 JSON 是合成 profile，只展示字段形状，不代表 OpenAI 或其他 Provider 的实际数值：

```json
{
  "input_forms": [
    "string",
    "string_array",
    "token_array",
    "token_array_array"
  ],
  "encoding": {
    "default": "float",
    "allowed": ["float", "base64"]
  },
  "dimensions": {
    "default": 1024,
    "allowed": {
      "kind": "values",
      "values": [256, 512, 1024]
    }
  },
  "limits": {
    "max_inputs": 32,
    "max_tokens_per_input": 8192,
    "max_total_tokens": 262144,
    "locally_counted_input_forms": [
      "token_array",
      "token_array_array"
    ]
  },
  "supported_parameters": ["dimensions", "encoding_format", "user"]
}
```

字段规则：

- `input_forms` 是非空、去重、确定排序的闭合枚举集合。
- `encoding.default` 是省略 `encoding_format` 时保证的 wire；`encoding.allowed = null` 表示客户端不能显式发送该字段，非空集合才可进入 `supported_parameters`。
- `dimensions.default` 是省略请求字段时保证的输出维度，必须为正且与 vector identity 一致。
- `dimensions.allowed = null` 表示客户端不能发送 `dimensions`；非空 domain 使用 `range` 或 `values`，不能用一个模糊 `supported: true`。
- `limits` 中 Provider 未证明的 token 值可以为 `null`；`null` 不表示无限，gateway request/JSON response/replay budget 始终另行存在。
- `locally_counted_input_forms` 明确哪些形状能在 egress 前执行 token limit；第一版只有 token arrays，string limits 仍由 Provider tokenizer 执行。
- `supported_parameters` 不重复必填 `model`/`input`，且只包含预检与 Native profile 已实现的顶层字段。
- 只有全部静态候选都允许的字段才能公开。字段出现在官方 schema 或某个首选 Route 中都不足以进入公共集合。

### 3.2 Canonical、API 与内部 identity

Canonical Embedding model 可拥有原生默认维度、tokenizer 和输入模态等模型事实。可变维度、encoding、批量与 served limits 属于具体 Upstream API。Public interface 是候选 API 的保证交集。

内部 `EmbeddingVectorIdentity` 至少覆盖：

```text
immutable model family/checkpoint
+ tokenizer or input encoding contract
+ default and requestable dimensions
+ normalization and distance contract
+ output encoding semantics
```

该 identity 只做相等性与 fallback 安全判断，不序列化到 Models API，也不能包含 credential、endpoint 或可反推私有拓扑的原始字符串。相同显示名、相同维度、同一 canonical family 或“OpenAI-compatible”标签都不是等价证明。

### 3.3 保守聚合

对同一 Embeddings Public Model：

1. 只编译静态启用且协议严格匹配的 Native candidate；任何 Bridge definition 启动失败。
2. `input_forms`、`encoding.allowed` 和 `supported_parameters` 求交集；显式 encoding 交集为空但 default 一致时令 `allowed = null` 并移除 `encoding_format`，default 不同则拒绝聚合。
3. `dimensions.allowed` 求区间/集合交集；结果为空但 default 一致时令 `allowed = null` 并移除 `dimensions` 参数，默认维度不同则拒绝聚合。
4. 数值上限取最小已保证值，并始终受 gateway 全局 request/JSON response limit 约束；有效 `max_inputs` 必须用 checked arithmetic 被显式 batch cap、最大公开维度、允许 encoding 的最坏序列化上界、固定 JSON envelope 与 response budget 收窄。若无法证明至少一个输入的合法成功响应受预算约束，registry 启动失败。
5. 多 candidate 只有 vector identity 完全相等时才能进入同一个 fallback group；当前首版实现限制为单 candidate。

### 3.4 当前唯一 checked-in 注册

当前编译目录已经形成下面一条客户端可执行链，而不是只存在合成 registry fixture：

| 层 | 固定注册 |
|---|---|
| Public Model | `embedding-primary` |
| Canonical/upstream model | `openai/text-embedding-3-small` / `text-embedding-3-small` |
| Provider / credential pool | OpenAI / `openai-primary` |
| Upstream Target | `openai-text-embedding-3-small`，仅绑定该 embedding model，不复用生成 target `openai-main` |
| Upstream API | `embeddings`，JSON `POST /v1/embeddings` |
| Route | `embedding-primary-openai-embeddings`，Public Model 仅有这一条 Embeddings → Embeddings Native candidate |

该 target 必须继续使用固定 `https://api.openai.com` trusted origin、现有 OpenAI credential pool 和 Provider header policy；请求不能覆盖 URL、path、model、credential、header 或 Route。一个 target 只绑定一个 upstream model，不能把 `text-embedding-3-small` 作为 `openai-main` 的请求期 model 分支。

初始 profile 只声明官方资料明确支持的事实：四种 input form、默认维度 1536、`float`/`base64` encoding、单输入 8192 tokens、单请求输入数组 2048 项和总计 300,000 tokens。当前没有从官方契约或经批准的真实 Provider 验收确认精确可变维度域，因此 `dimensions.allowed = null`，`supported_parameters` 只有 `encoding_format` 与 `user`；不得把“可以缩短”推断成未经证明的整数区间。公开 `max_inputs` 还会被 bootstrap 的 JSON response budget 进一步收窄。

## 4. 请求分析与预检

建议的确定处理链：

1. 认证并要求唯一兼容的 JSON `Content-Type`，在解析前应用 `max_request_body_bytes`。
2. 解析单个 JSON object，提取 Public Model，并拒绝 stream、generation 字段及未知 endpoint 字段。
3. 按元素类型冻结 `input_form`、batch count、每个 token-array 长度、总 token-array token 数和原始请求字节。
4. 校验非空、集合能力、显式或默认 encoding、dimension domain、可直接计算的 token/batch limit 与响应预算。
5. 读取同一个预编译 Embeddings execution interface；预检不得扫描 Route 或根据请求重新选择 candidate。
6. planning 按固定顺序产生唯一 Native candidate；adapter 只绑定受信 path/auth/header 并把 Public Model 改写为 upstream model。

第一版不为 string 输入实现 tokenizer。因而：

- 字符串只做结构、UTF-8/JSON、batch 与字节限制；
- token array 可以确定执行 per-input/total token count；
- 字符串 token 上限由目标 Provider 执行，真实 profile 验收必须记录该边界；
- 不得用字符数、UTF-8 字节数或估算比例伪装成 token 计数。

## 5. 响应与下游投影

Embeddings 是有界单个 JSON response，不进入 SSE lifecycle。bootstrap schema 必须把三种用途拆开：

- `max_request_body_bytes`：下游入站 request hard limit；
- `max_json_response_body_bytes`：非流式 JSON 成功体在首次下游 commit 前的读取上限；
- `max_replay_body_bytes`：请求是否可以再次发送的资格上限，且必须小于或等于 request limit。

三个字段都是必填正整数，不使用相互回退的默认值。现有 Chat/Responses 把 `max_request_body_bytes` 传给 JSON response cache 的位置必须改用 response 字段，避免同一个配置名继续承担两种语义。Embeddings response validator 应在提交下游前：

- 检查成功 media type 与响应大小；
- 解析并验证顶层 object/data/model/usage；
- 验证 `data.len()` 与输入项数一致且每个 `data[i].index == i`；不通过重排来修复非法上游响应；
- 对 `float` 验证每项为有限 JSON number array 且维度符合请求/默认值；
- 对 `base64` 验证字符串及解码长度与维度/profile 一致，但返回原字符串；
- 将响应 `model` 改写为下游 Public Model，避免泄漏真实 upstream model；
- 保持 vector、index、object 和 usage 的值语义，不把向量写入诊断。

若成功体 media type、JSON、index、编码、维度或 usage 不符合 contract，应返回稳定、安全的 upstream protocol error；不能把半个成功体提交后再 fallback。

## 6. Retry、取消、错误与遥测

- 单 candidate 可在响应提交前按现有 attempt budget 处理可重试 transport/429/5xx；只有 body 不超过 `max_replay_body_bytes` 才能执行第二次 attempt。
- body 超过 replay budget 但仍在 request limit 内时，合法请求只执行第一次 attempt；不因为内部 replay 优化而返回新的客户端输入错误。
- 下游取消停止当前发送/接收、credential 等待与 backoff。
- 当前实现不提供多 Embeddings Route；后续即使实现，也必须先证明 vector identity 等价。
- 上游错误只允许保留安全 status、request id 与 allowlist rate-limit header；不得回传 upstream body、endpoint、credential 或私有诊断。

错误响应必须使用 `{ "error": { "message", "type", "param", "code" } }`，并按下表固定：

| 条件 | HTTP | `type` | `code` | `param` | attempt/egress |
|---|---:|---|---|---|---|
| 非 JSON object、缺失/空 `model`、未知字段、非法/空/混合 input union、非法 token、未知 enum 值 | 400 | `invalid_request_error` | `invalid_request_error` | 能唯一定位时为标准字段名，否则 `null` | 0 |
| Public Model 未知、retired 或不可见 | 404 | `invalid_request_error` | `model_not_found` | `model` | 0 |
| Public Model 没有 Embeddings interface，或合法 input form/encoding/dimension/batch/token 要求超出固定契约 | 400 | `invalid_request_error` | `unsupported_model_capability` | 对应标准字段；接口缺失时为 `model` | 0 |
| 非 JSON `Content-Type` | 415 | `invalid_request_error` | `unsupported_media_type` | `null` | 0 |
| 入站 body 超过 `max_request_body_bytes` | 413 | `invalid_request_error` | `request_too_large` | `null` | 0 |
| body 超过 replay budget 但仍合法 | 无新增错误 | 无 | 无 | 无 | 恰好 1 次 attempt |
| upstream 成功体 media type、大小、JSON、object、model、data/index、encoding、dimension 或 usage 非法 | 502 | `server_error` | `invalid_upstream_response` | `null` | 当前 attempt 后结束，不 retry |
| transport timeout 且 attempt 用尽 | 504 | `server_error` | `upstream_timeout` | `null` | 不超过 attempt budget |
| 其他 transport failure 且 attempt 用尽 | 502 | `server_error` | `upstream_error` | `null` | 不超过 attempt budget |

Provider 返回的非成功 HTTP 状态继续服从共享 Provider retry/status contract，Embeddings 不重定义其状态分类；但该路径不得把非成功 body 交给成功响应 validator，也不得新增包含 upstream model、endpoint 或 payload 的错误文本。

可观测性直接使用独立 `OperationKind`，不把 Embeddings 塞进 generation-only `ApiProtocol`。现有 request/attempt snapshot 和低基数 label 中语义为 `protocol` 的字段原子迁移为 `operation`，不保留两套字段；三个取值固定为 `chat_completions`、`responses`、`embeddings_create`。Embeddings 只记录：

- request/attempt 数、状态、取消、重试和总耗时等既有低基数数据；
- `usage.prompt_tokens` → input tokens，`usage.total_tokens` → total tokens；
- `output_tokens = None`，且不产生 generation tokens-per-second 或伪造 SSE/terminal 样本。

`user`、原始文本、token array、完整 request/response body、向量和 base64 不进入普通日志、trace event、错误 message、metrics label 或 snapshot。`observability_contract` 必须用带哨兵值的 fixture 断言这些值不会出现在导出结果中，并验证 `operation = embeddings_create`、input/total 累计值、output/throughput 缺失以及 replay 超限时只有一次 attempt。

## 7. TDD 与验收矩阵

| 层 | 已建立的契约与继续适用的验收行为 |
|---|---|
| definition/registry | 独立 task/API/Route/interface；维度域和集合校验；generation/Bridge 错配启动失败 |
| Models API | 标准四字段不变；扩展 `interfaces.embeddings` 精确投影且不泄漏 topology/vector identity |
| ingress/request | 认证、Content-Type、body limit、四种 union、空/混合输入、未知 model/field、encoding/dimensions gate |
| adapter | 只使用受信 endpoint/credential/upstream model；请求不能选择 URL、header、Route 或 identity |
| response | float/base64、public model 投影、data 顺序/index、维度、usage、media type 和 body limit |
| resilience | 单 Route 429/5xx/timeout 的有限 attempt、replay 超限只执行一次、取消停止、首个 response commit 后不重放 |
| errors | 上表每行的 status/type/code/param、attempt/egress 和无敏感回显均逐项断言 |
| observability | `operation=embeddings_create`、input/total usage、无 output/throughput，以及文本/token/`user`/vector/base64 哨兵不泄漏 |
| client | 独立 Python/OpenAI SDK 或 curl 通过 loopback 调用 public endpoint；版本与未运行层明确记录 |

确定性 Rust/mock 测试只证明本地 registry、routing 与 wire 行为。真实模型可用性、字符串 token 限制、默认维度、归一化、向量质量、费用和跨服务等价性必须由经批准的 Provider 测试单独证明。

## 8. 当前实现的非目标

- 不增加第二个 Provider/model/Route candidate，也不复用生成模型 target 做请求期 model 选择；
- 不做跨 Provider 向量转换、降维、归一化、缓存、索引或检索；
- 不实现 streaming、异步 job、Batch、Vector Store 或 embedding Bridge；
- 不把当前 OpenAI 限制值提升为所有 Provider 的全局默认；
- 不以真实 Provider 调用替代 deterministic contract tests。
