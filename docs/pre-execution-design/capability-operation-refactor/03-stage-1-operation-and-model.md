# 03：阶段 1——Operation kernel 与 task-explicit API

## 目标

移除 Provider/runtime 中固定三 operation 字段和 operation→task 隐式推断，同时保留单 task canonical profile 及现有下游 wire 行为。

## 依赖

- 阶段 0 的格式、测试和 deny-all builders 基线全绿；
- Operation、task、modality、capability 与 resource affinity 的边界已冻结；
- private operation set 与 typed API key 形状已确定。

## Direct replacement

1. `OperationKind` 保持 closed enum；Provider ceiling、executable profile 与 runtime API 改为 operation-tagged set。
2. `ApiProtocol` 只表示可进入 Generation Bridge 的 Chat/Responses 协议对。
3. `ModelConfig.task: CanonicalModelTask` 保持单值，task-specific facts 继续由 variant 独占。
4. `UpstreamApiConfig` 使用 typed `(operation, task)` key；task binding 必须与 canonical profile 一致。
5. Compiler 先应用该 task 的 model rules，再把一个 selected task profile 写入 runtime `UpstreamApi`。
6. 每个 private operation interface 显式保存 task；task-sensitive policy 不再依赖 Public Model 全局 shortcut。
7. Route transform 收紧为 `Native | GenerationBridge(direction)`；非 Generation operation 不能构造 Bridge。
8. 所有 Provider registration、runtime lookup、Route binding、builders 和 fixtures 原子迁移，旧 representation 当阶段删除。

## 先失败测试

- duplicate Provider operation ceiling 失败；
- duplicate `(operation, task)` API key 失败；
- API task 与 canonical profile 不一致失败；
- operation/profile/task variant 不一致失败；
- Route 引用缺失 key 或 Native operation 不一致失败；
- 非 Generation route 使用 Bridge 失败；
- 同一 Public operation interface 混合不兼容 task candidate 失败；
- Generation instructions/reasoning policy 只从当前 interface task 获取。

## 实施步骤

1. 新增 definition、key、compatibility matrix 和 selected-profile RED tests；
2. 替换 core/provider operation representation；
3. 替换 registry config、runtime index 与 Route reference；
4. 迁移 models/providers catalog 和 test builders；
5. 把 task-sensitive compile facts 下沉到 operation interface；
6. 删除 operation-only key、固定字段 accessor、宽泛 Bridge mode 和迁移转换；
7. 全仓库搜索 legacy symbol 后运行完整基线。

## 退出门

- Chat、Responses、Embeddings 的 standard wire、Models v1、preflight 和 Route 顺序不变；
- runtime `UpstreamApi` 只持有一个已选择的 task profile；
- exhaustive match 不使用掩盖新 operation/task 的 wildcard；
- focused tests、完整 Rust 基线和 `git diff --check` 通过。

## 非目标

- 不引入 `CanonicalTaskSet`、共享 `ModelIdentity` 或跨 task Public Model；
- 不实施 media profile、Models v2、operation-first pipeline 或新 endpoint。
