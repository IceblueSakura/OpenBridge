# 05：阶段 3——Operation-indexed Registry 与稳定 Models 投影

## 目标

把 private Public Model execution snapshot 从固定 Chat/Responses/Embeddings 字段升级为 operation-indexed interfaces，同时保持标准与扩展
Models 的当前公共 schema 不变。

## 依赖

- 阶段 1 的 typed `(operation, task)` key 和 selected task profile 已稳定；
- 阶段 2 的 media envelope、subset/intersection 和显式 Target profile 已稳定；
- 当前 Models v1 canonical fixture 与 OpenAPI 合同全绿。

## Direct replacement

1. `ModelExecutionInterfaces` 改为 `BTreeMap<OperationKind, OperationExecutionInterface>` 或等价 deterministic closed set。
2. 每个 interface 同时拥有：
   - 单一 task binding；
   - private executable operation contract；
   - 产生该 contract 的固定 candidate 顺序；
   - Native/GenerationBridge transform；
   - resource/continuation affinity；
   - operation-specific response budget。
3. Compiler 按 operation 生成 contribution、聚合和可达性校验，不在根模块按 Provider 名称分支。
4. 当前 Public Model 继续只聚合一个 canonical task；同一 interface 的全部 candidates 必须 task-compatible。
5. Standard Models 与扩展 Models v1 只从 private map 投影；preflight 不读取 DTO。
6. 现有 `native_protocol` filter 通过 private candidate predicate 实现，不泄露 topology 或改变 Route 顺序。

## 先失败测试

- duplicate operation interface 或同一 interface 混合 task 失败；
- interface 的 contract、task 和 candidate list 同步生成；
- candidate 顺序变化不改变能力交集，但改变 execution order；
- operation 缺少 Native coverage 时只有允许的 Generation Bridge 才可补充；
- standard/extended Models list/retrieve 与阶段前 v1 fixture 完全一致；
- Models 投影不包含 Provider、Target、Route、upstream model、credential 或 private affinity。

## 删除清单

- private fixed `chat_completions/responses/embeddings` execution fields；
- operation-only、generation-only interface accessor；
- 从 Public DTO 反向读取 capability 或 planning facts 的路径；
- 为旧/new private snapshot 保留的 alias、conversion 或 dual representation。

## Models v2 触发门

本阶段不发布 v2。只有现有 v1 无法准确表达已批准客户端合同时，才单独建立 current focus；真实新 operation 或跨 task Public Model 只
触发重新评估。届时定义 operation names、task mapping、query、缺失语义和 schema version 类型，并原子替换 DTO、OpenAPI、examples、
fixtures、tests 与 requirements。不得提供 v1/v2 双输出、alias 或 content negotiation shim。

## 退出门

- standard Models 与扩展 Models v1 JSON shape 合同不变；
- preflight/planning 读取同一 private operation interface；
- interface 的 task、contract 与 candidate 顺序同源；
- topology privacy、startup validation、compiled Models、config contract 和完整 Rust 基线通过。

## 非目标

- 不实现 Models v2、新 endpoint、跨 task Public Model、动态 Provider discovery 或 attempt engine 重写。
