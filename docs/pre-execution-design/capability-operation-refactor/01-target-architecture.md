# 01：目标架构

## 1. 总体数据流

```text
models/                         providers/
Canonical identity + task set  Provider operation ceilings + closed adapters
              \                 /
               registry compiler
        Upstream API executable profile
                    ↓
           Route contribution
                    ↓
 PublicModel OperationInterface { contract, fixed candidates }
                    ↓
ingress → analyze → preflight → plan → attempt → adapter → transport → renderer
```

同一 `OperationInterface` 必须同时服务扩展 Models 投影、preflight 和 planning，避免三份能力事实漂移。

## 2. Operation kernel

把 `OperationKind` 作为最高级闭合分发边界；`ApiProtocol` 只保留为 Chat/Responses Generation Bridge 的内部协议对。

建议结构：

```text
src/core/operation/
  mod.rs
  kind.rs
  capabilities.rs
  request.rs
  response.rs
  generation/
  embeddings/
  images/        # 只有明确实施对应 endpoint 时才创建
  audio/         # 同上
  files/         # 同上
```

Provider ceiling 和 executable profile 使用 operation-tagged closed enum，而不是在 `ApiCapabilities` 中不断增加可选字段，也不使用 string map：

```rust
pub enum ProviderOperationCeiling {
    ChatCompletions(ProviderChatProfile),
    Responses(ProviderResponsesProfile),
    EmbeddingsCreate(ProviderEmbeddingsProfile),
}

pub enum ExecutableOperationProfile {
    ChatCompletions(ChatProfile),
    Responses(ResponsesProfile),
    EmbeddingsCreate(EmbeddingsProfile),
}
```

只有进入实施范围的 operation 才新增变体，禁止提前放置空 handler 或 feature flag。

## 3. Canonical task set

当前单 task 假设应替换为非空、唯一的闭合 task set：

```rust
pub struct ModelConfig {
    pub identity: ModelIdentity,
    pub tasks: Vec<CanonicalTaskProfile>,
}

pub enum CanonicalTaskProfile {
    Generation(GenerationModelFacts),
    Embedding(EmbeddingModelFacts),
    SpeechRecognition(SpeechRecognitionModelFacts),
    SpeechSynthesis(SpeechSynthesisModelFacts),
    VoiceDesign(VoiceDesignModelFacts),
    VoiceClone(VoiceCloneModelFacts),
}
```

约束：

- 一个 canonical model 可拥有多个不同 task kind；同 kind 不得重复；
- `UpstreamApiConfig` 必须显式绑定一个 canonical task；
- 同一 Public Model operation 的全部 candidate 必须绑定可聚合的同一 task kind；
- 同一 Public Model 可在不同 operation 暴露不同 task；
- 若两个不兼容 task 共用同一个下游 operation，应使用不同 Public Model identity，不能按请求 shape 动态选择 task。

## 4. Media capability layer

```text
src/core/capability/generation/media/
  mod.rs       # Chat/Responses media envelope
  common.rs    # MaxParts、URL/inline byte limits、checked constructors
  image.rs
  audio.rs
  file.rs
```

Operation profile 持有完整 media envelope：

```rust
pub struct ChatMediaProfile<A> {
    pub image: Option<ImageInputProfile>,
    pub audio: Option<A>,
    pub file: Option<ChatFileInputProfile>,
}

pub struct ResponsesMediaProfile {
    pub image: Option<ImageInputProfile>,
    pub file: Option<ResponsesFileInputProfile>,
}
```

- image 保留 source-owned discriminated union；
- audio 改为每个 source 自己拥有 format/limits payload，不再以零值表达 source 不存在；
- file 替换 `file_input: bool`，分别建模 Chat 和 Responses wire；
- voice 继续属于 audio task，不新增顶层 voice medium；
- resource-backed image/file ID 在 issuer/owner/affinity 完整前保持 unsupported。

## 5. Provider-local catalog

```text
src/providers/<provider>/
  definition.rs       # provider identity、adapter 与 ceiling assembly
  media.rs            # Provider ceiling + 具名 executable Target profiles
  registration.rs     # Target/API binding
```

Target 必须显式传入完整 executable media profile；`to_executable()` 不再自动继承 Provider media ceiling。跨 Provider 只共享 constructor 和算法，不共享事实 constant。

## 6. Registry 与 Public Model

```rust
pub struct OperationExecutionInterface {
    pub contract: PublicOperationContract,
    pub candidates: Vec<RouteExecutionCandidate>,
}

pub struct ModelExecutionInterfaces {
    pub operations: BTreeMap<OperationKind, OperationExecutionInterface>,
}
```

每个 operation 独立贡献、验证和聚合。保守交集不能仅逐字段求交；最终 profile 必须重新执行可达性和组合一致性校验。

## 7. Operation-first pipeline

```text
src/pipeline/
  generation/{analysis,preflight,planning,response}.rs
  embeddings/{analysis,preflight,planning,response}.rs
  <future-operation>/...
  execution/{attempt,retry,commit,cancellation}.rs
```

operation 模块拥有请求 shape、limits、response framing 和错误；共享 execution 层只拥有首输出前 attempt、credential、retry/fallback、commit 和取消生命周期。避免一个万能 pipeline 解释所有 operation。

## 8. Models v2

标准 `/v1/models` 保持四字段。扩展接口可直接替换为 operation-indexed v2：

```json
{
  "schema_version": 2,
  "id": "public-model",
  "model_facts": {
    "tasks": []
  },
  "interfaces": {
    "chat_completions": {},
    "responses": {},
    "embeddings_create": {}
  }
}
```

`model_facts` 只表达聚合后的模型事实；`interfaces` 表达可执行合同。不得复制 execution topology，也不得从 v2 DTO 反向构造 private contract。
