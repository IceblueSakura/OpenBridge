# 当前开发焦点

## 状态

**活动焦点：一个可发现、可调用且可验证的 Native Embeddings 接口。** 本文只授权下面一个客户端可观察行为；完成后把已证明事实写入 implementation status，并将本文恢复为空焦点。

Chat/Responses Native 多模态仍是已批准的功能需求，但不属于本轮活动焦点。只有本焦点完成、验证并清空后，才能把其中一个独立多模态行为写入新的 `current-focus.md`；不得与 Embeddings 并行实现。

## 可观察行为

已认证客户端从 `GET /openbridge/v1/models` 读取 Public Model `embedding-primary` 的 `interfaces.embeddings` 固定契约后，按该契约调用 `POST /v1/embeddings`：

- 合法请求只进入与该 execution interface 绑定的一条受信 OpenAI Native Route；
- 网关把 Public Model 改写为该 Route 固定的 upstream model，并在成功响应中恢复 Public Model；
- input form、encoding、dimension、batch/token/body 限制由 Models projection 与请求 preflight 共用同一份预编译契约；
- 可在本地判定的非法或不支持请求在第一次 upstream 调用前稳定失败；
- 成功响应在提交下游前完成有界读取和结构校验，向量、顺序、index、encoding 与 usage 语义不被转换。

只完成 schema、Models 展示、Router 路径、Provider 透传或未注册的合成 fixture，均不算完成该行为。

## 对应功能需求

- [Embeddings 与 Native 多模态扩展需求](../functional-requirements/embedding-and-native-multimodal.md)：本轮只实施 EXT-01、EXT-02、EXT-03，以及 EXT-08/EXT-09 中直接适用于 Embeddings 的边界。
- [Public Model 与模型能力契约](../functional-requirements/model-information-and-capability-contract.md)：固定保守接口、Models/preflight 同源、禁止请求期 capability routing。
- [路由与 Provider 韧性](../functional-requirements/provider-resilience.md)：固定候选、响应提交、取消与有限 retry。
- [Embeddings 实现细节](../references/openai/implementation-details/01-embeddings.md)。

## 先失败的测试或复现

实现前按下列阶段逐个建立失败证据。每个阶段只能解决列出的一个问题；新增测试转绿、直接回归通过并复核差异后，才能开始下一阶段：

1. `embedding_definition_contract`：当前类型和 compiler 不能表达独立 Embedding interface 及其 Models projection。
2. `embedding_forwarding_contract` 的 request/egress case：当前 Router、preflight、planning 和 adapter 不能完成 Embeddings Native 请求链。
3. `embedding_forwarding_contract` 的 bounded-response case 与 `config_contract`：当前没有独立 JSON response budget 或完整成功体 validator。
4. `embedding_forwarding_contract` 的 replay/cancel case：当前没有独立 replay eligibility 与单 Route attempt 断言。
5. `embedding_forwarding_contract` 的 error-matrix case：当前缺少精确 status/type/code/param 与 zero-egress/attempt 断言。
6. `observability_contract` 的 Embeddings case：当前 attempt/usage 维度不能独立标识 Embeddings，也不能证明敏感输入和向量未进入普通遥测。
7. `embedding_registry_contract`：当前 checked-in registry 没有 `embedding-primary`、独立 OpenAI target/API 和单条 Native Route。
8. 独立 curl/Python loopback：当前没有从 Models discovery 到 Embeddings 成功响应的客户端可见闭环。

## 分阶段最小实现边界

### 阶段 1：独立契约与 compiler

- 增加独立 Embedding task、operation、Upstream API capability、Native Route 和 Public Model interface；不得把它塞入 Chat/Responses generation capability 或 Bridge。
- `interfaces.embeddings` 明确表达四种 input form、encoding default/allowed、dimension default/allowed、limits、`locally_counted_input_forms` 与顶层 `supported_parameters`。
- registry compiler 校验闭合集合、默认值、domain、限制和单 candidate，并让 Models projection 与 preflight 持有同一个不可变 execution interface。
- 本阶段只使用合成 registry fixture 证明类型、compiler 和 Models projection；不增加 ingress 路由或 checked-in Public Model。

### 阶段 2：请求与 Native egress

- 用合成 registry fixture 注册受保护、JSON-only、非流式的 `POST /v1/embeddings`，加入 endpoint-specific request analysis、preflight、planning 与 adapter；checked-in catalog 仍不公开 Embeddings。
- 支持 string、string array、token array、token-array array；拒绝空值、混合或歧义形状、非法 token、stream、generation 字段和未知顶层字段。
- 省略或显式 `encoding_format`/`dimensions` 都服从固定 interface；第一版不为字符串实现 tokenizer，只对 token-array 两种形状执行精确 token count。
- adapter 只改写受信 path/model/auth/header；客户端请求不能选择 Provider、URL、header、credential、upstream model 或 Route。
- 本阶段只证明请求与 egress bytes；成功体尚未通过阶段 3 validator 时不得把 built-in Public Model 暴露为可调用。

### 阶段 3：有界成功响应

- bootstrap schema 增加必填且非零的 `max_json_response_body_bytes`。`max_request_body_bytes` 此后只表示下游入站 body 上限，现有把它当 JSON response cache 上限的调用点必须迁移到 response 字段。
- 非流式成功响应必须在首次下游 commit 前按 `max_json_response_body_bytes` 有界读取；校验 media type、JSON、object、data 数量/顺序/index、encoding、维度、model 与 usage。超限成功体不得截断、透传或 retry。
- registry compiler 使用 checked arithmetic，从 response budget、最大公开维度、允许 encoding 的最坏序列化上界和固定 JSON envelope 推导有效 `max_inputs`；无法证明至少一个输入的合法成功响应受预算约束时启动失败。
- validator 将 upstream model 投影回 Public Model，并保持 vector/base64、顺序、index、object 与 usage 的值语义。

### 阶段 4：重放、attempt 与取消

- bootstrap schema 增加必填且非零的 `max_replay_body_bytes`，并校验 `max_replay_body_bytes <= max_request_body_bytes`；不从 request/response limit 推导默认值。
- body 不超过 `max_replay_body_bytes` 的请求才可在首个 response commit 前执行 retry；更大的合法请求只执行第一次 attempt，而不是因内部 replay 优化被额外拒绝。
- 当前单 Route 只沿用有限 credential/transport/429/5xx attempt budget，不增加跨 Provider/模型 fallback；下游取消停止当前发送、接收、credential 等待和 backoff。
- 本阶段只修改 replay eligibility、attempt/cancel 状态机和对应断言，不改变错误 envelope 或遥测 schema。

### 阶段 5：错误契约

- 按 [Embeddings 错误矩阵](../references/openai/implementation-details/01-embeddings.md#6-retry取消错误与遥测) 固定 status/type/code/param、attempt 和 egress。
- 所有本地拒绝断言 upstream 调用次数为零；非法或超限成功体在当前 attempt 后返回安全错误，不回显 upstream body 或 topology。
- 本阶段只修改错误分类、响应构造和对应 fixture，不迁移指标字段。

### 阶段 6：可观测性

- 将可观测性中的 `protocol` 语义直接迁移为 `operation`，Embeddings 固定使用低基数字符串 `embeddings_create`。
- usage 只把 `prompt_tokens`/`total_tokens` 计入 input/total counter，不产生 output-token 或 generation-throughput 样本，也不记录文本、token array、`user`、向量、base64 或完整 body。
- 本阶段只修改 request/attempt observation、snapshot/metrics schema 和 `observability_contract`。

### 阶段 7：checked-in 可执行注册

- 仅在阶段 1-6 全部通过后，注册 Public Model `embedding-primary`，由一条受信 OpenAI Native Route 固定指向独立的 `text-embedding-3-small` target 与 `/v1/embeddings`。
- 同步 Provider contract、canonical model、target/API、Route、Public Model 和 compiled-registry tests；不得复用 `openai-main` 做请求期 model 分支。
- 本阶段只把已完成的垂直链路接入 built-in catalog，不新增第二 candidate 或动态发现。

### 阶段 8：协议发布与完成验证

- 同步 `docs/openapi.yaml`、功能需求、实现细节、bootstrap 配置示例和契约测试。
- 用独立 curl 或 Python loopback 从 Models discovery 到 Embeddings 成功响应完成客户端可见闭环；外部 OpenAI SDK 只在依赖已可用或用户批准安装时运行。
- 完成全部聚焦测试和 Rust baseline 后，只把实际证据写入 implementation status，再清空本文件。

## 首版最佳实践迁移规则

扩展 Models schema 与 Embeddings endpoint 尚未发布，本轮直接修正首版契约，不承担向后兼容：

- 扩展 Models 继续使用 `schema_version: "1"`，直接增加分型的 `interfaces.embeddings`；不创建 v2、旧字段镜像、legacy DTO、兼容 alias 或双写 projection。
- Embeddings 使用独立 operation/task/capability/Route/request/response 类型；不在 `ApiProtocol` 或 generation DTO 中保留布尔占位、兼容 variant 或转换 shim。
- bootstrap 的两个新预算字段是必填项；同步修改 `config/bootstrap.toml`、`config/bootstrap.example.toml` 和全部测试 fixture。缺字段、零值或非法大小关系必须启动失败，不回退到 `max_request_body_bytes`。
- 请求 parser 只接受本轮 OpenAPI 中的标准字段和形状；不接受旧名、拼写别名、宽松 union、双读字段或静默删除未知字段。
- 本轮不顺带迁移 Chat/Responses 的 reserved media bool。后续多模态焦点必须在自己的阶段中原子替换相关公共/内部契约，不保留 bool 与 typed sub-capability 并行的兼容期。

## 明确不做

- 不实现 Chat/Responses Native 多模态、Chat ↔ Responses 多模态 Bridge 或任何 reserved audio/file capability；
- 不实现多个 Embeddings candidate、跨 Provider/模型 fallback、vector identity 等价聚合或 embedding Bridge；
- 不实现向量转换、归一化、降维、缓存、索引、检索、Vector Stores/File Search 或 Batch；
- 不实现 string tokenizer、streaming、异步 job、动态 capability routing 或 Provider discovery；
- 不实现旧 schema/config/request 的兼容读取、弃用窗口、双写或迁移 adapter；
- 不用真实 Provider 调用替代 deterministic contract tests，也不把未运行的 SDK、真实 Provider、负载或长期测试写成已验证。

## 验证顺序

每个阶段先运行该阶段新增测试，再运行其直接影响的既有契约；不得把所有失败留到最后一次 baseline。聚焦命令固定为：

```powershell
cargo test --locked --test embedding_definition_contract
cargo test --locked --test embedding_registry_contract
cargo test --locked --test embedding_forwarding_contract
cargo test --locked --test capability_definition_contract
cargo test --locked --test provider_contract
cargo test --locked --test provider_boundary_contract
cargo test --locked --test config_contract
cargo test --locked --test example_config
cargo test --locked --test ingress_contract
cargo test --locked --test native_routing_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test observability_contract
```

完成全部阶段后执行：

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

若修改 `testdata/` 或 `tools/corpus/`，再执行：

```powershell
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

确定性 Rust/mock/loopback 只证明本地 registry、routing、wire、错误和取消边界。真实 OpenAI model 可用性、string token 上限、维度/编码当前接受域、费用、向量质量和长期稳定性属于另行批准的外部验收层。

## 完成判定

只有以下条件同时成立才可完成焦点：

- `embedding-primary` 在 checked-in registry 中具有一条真实可执行的受信 Native Route，而不是只有合成测试定义；
- 扩展 Models、preflight、planning 和 forwarding 使用同一个 Embeddings execution interface；
- 四种 input form、公开 encoding/dimension domain、model 双向投影、float/base64、data/index/object/usage 与限制行为通过；
- 所有 zero-egress、response validation、budget、retry/cancel、错误与遥测保护 case 通过；
- OpenAPI、配置、需求、实现细节与实际首版契约一致，且实际运行的 baseline 通过；
- implementation status 只记录已运行证据，本文件随后恢复为空焦点。
