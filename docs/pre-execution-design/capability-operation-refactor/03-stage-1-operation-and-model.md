# 03：阶段 1——Operation kernel 与多 task Model

## 目标

移除“只有 Chat/Responses/Embeddings 三个固定字段”和“每个 canonical model 只有一个 task”的长期扩展瓶颈，同时保持现有三个 operation 的下游 wire 行为不变。

## 依赖

- 阶段 0 的格式、测试和 synthetic builders 基线全绿；
- [目标架构](01-target-architecture.md)中的 Operation/Task/Modality/Capability/Resource 术语已确认；
- Models v2 的详细 JSON 尚可延后，但 private operation set 形状必须确定。

## Direct replacement

1. `OperationKind` 继续作为 closed enum，但 provider/runtime interface 改为 operation-tagged set，不再由 `ApiCapabilities` 固定字段拥有。
2. `ApiProtocol` 收缩到 Generation Bridge；Embeddings 和未来 operation 不再借用 generation request 类型。
3. `CanonicalModelTask` 单 variant 替换为 non-empty unique `CanonicalTaskSet`。
4. `UpstreamApiConfig` 增加显式 `task_binding`，operation 与 task 分别校验。
5. Route transform 从笼统 `Native/Bridged` 收紧为 `Native | GenerationBridge(direction)`；非 Generation operation 不能构造 Bridge。
6. 所有模型、Provider ceiling、Target/API 和 synthetic fixtures 原子迁移到新类型；旧类型当阶段删除。

## 先失败测试

- duplicate operation ceiling 在启动前失败；
- duplicate canonical task kind 在启动前失败；
- API 绑定不存在的 model task 失败；
- operation/profile variant 不一致失败；
- 非 Generation route 使用 Bridge 失败；
- synthetic multi-task canonical model 可让不同 operation 各自绑定 task；
- 同一 Public Model operation 混合不兼容 task candidate 失败。

## 实施步骤

1. 先新增 pure definition/validation tests；
2. 替换 core operation/task definitions；
3. 替换 registry config 与 runtime entity；
4. 迁移 models/providers catalog；
5. 更新 compiler validation；
6. 更新 tests/support builders；
7. 删除旧 enum、field accessor、conversion helper 和 wildcard match；
8. 全仓库搜索 legacy symbol。

## 删除清单

- 固定字段式 `ApiCapabilities`；
- 单值 `CanonicalModelTask` owner 规则；
- 全局 generation/embedding request 特化中不再需要的 wrapper；
- 由 downstream/upstream operation 隐式猜测 Bridge direction 的分支；
- 为迁移保留的 alias、From 转换或双 representation。

## 退出门

- 当前 Chat、Responses、Embeddings 的 standard wire、Models visibility、preflight 和 Route 顺序不变；
- 新 validation focused tests 全绿；
- 所有 exhaustive matches 无 wildcard；
- 完整 Rust 基线与 `git diff --check` 通过；
- 文档不宣称任何未来 operation 已实现。

## 非目标

- 不实施 media profile 重构；
- 不发布 Models v2；
- 不拆 operation-first pipeline；
- 不新增真实 endpoint。
