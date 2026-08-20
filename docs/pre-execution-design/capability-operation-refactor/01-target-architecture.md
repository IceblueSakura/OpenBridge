# 01：目标架构

## 1. 总体数据流

```text
models/                                  providers/
Canonical single-task profiles          operation ceilings + closed adapters
                    \                    /
                     registry compiler
             UpstreamApiKey(operation, task)
                 + selected task profile
                            ↓
                   Route contribution
                            ↓
 OperationInterface { task, contract, fixed candidates }
                            ↓
ingress → analyze → preflight → plan → execution → adapter → transport → renderer
```

同一 private `OperationInterface` 必须同时服务 Models 投影、preflight 和 planning，避免能力、task 与候选事实漂移。

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

## 3. Single-task executable profile

Canonical profile 继续由一个闭合 task variant 独占 task-specific facts：

```rust
pub struct ModelConfig {
    // Existing canonical identity metadata remains on this profile.
    pub task: CanonicalModelTask,
}

pub struct UpstreamApiKey {
    pub operation: OperationKind,
    pub task: CanonicalTaskKind,
}
```

约束：

- `UpstreamApiConfig` 显式绑定 task，且必须与引用的 canonical profile 一致；
- compiler 先选择并收窄 task profile，再生成 runtime `UpstreamApi`；请求路径不携带 task set；
- 每个 Public Model operation interface 显式保存 task，其全部 candidates 必须 task-compatible；
- task-sensitive policy 属于 operation interface，不能由 Public Model 全局 bool 推断；
- 同一 operation 下的不兼容 task 使用不同 Public Model identity，不能按 request shape 动态选择。

本轮不引入共享 `ModelIdentity + TaskProfile[]`。只有同一真实模型跨 task 重复注册并产生 identity 漂移、同一 Target 确需多 task，或一个
Public Model 必须跨 operation 暴露不同 task 时，才单独设计该层；compiled API 仍只保存一个 selected task profile。

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
    pub task: CanonicalTaskKind,
    pub contract: PublicOperationContract,
    pub candidates: Vec<RouteExecutionCandidate>,
}

pub struct ModelExecutionInterfaces {
    pub operations: BTreeMap<OperationKind, OperationExecutionInterface>,
}
```

每个 operation 独立贡献、验证和聚合。内部 map 不改变 Public Model 当前单 task 合同；保守交集完成后必须重新执行可达性和组合一致性校验。

## 7. Operation-first pipeline

```text
src/pipeline/
  generation/{analysis,preflight,planning,response}.rs
  embeddings/{analysis,preflight,planning,response}.rs
  <future-operation>/...

src/execution/
  coordinator.rs
  retry.rs
  commit.rs
  cancellation.rs
```

`pipeline/` 拥有 operation-specific 的纯 request/response 语义、preflight 与 planning，不执行 I/O。顶层 `execution/` 只拥有 fixed
candidate traversal、credential、retry/fallback、commit 和取消生命周期。

## 8. Models projection

标准 `/v1/models` 保持四字段；扩展 `/openbridge/v1/models` 暂时保持当前唯一 schema v1。Private operation map 通过显式投影生成现有固定
DTO 字段，preflight 不读取 DTO，也不建立 v1/v2 双输出。

只有 schema v1 无法准确表达已批准客户端合同时，才单独定义并直接替换 Models v2；真实新 operation 或跨 task Public Model 只是
重新评估时点，不自动触发切换。届时 DTO、OpenAPI、examples、fixtures、tests 和 requirements 必须原子更新，不保留 alias 或 shim。
