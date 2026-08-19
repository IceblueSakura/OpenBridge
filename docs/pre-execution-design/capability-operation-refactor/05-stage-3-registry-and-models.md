# 05：阶段 3——Registry compiler 与 Models v2

## 目标

把 Public Model 从固定 Generation/Embeddings 字段升级为 operation-indexed execution interfaces，并直接替换扩展 Models schema，使未来新增 operation 不再要求在 registry 根类型中增加平铺字段。

## 依赖

- 阶段 1 的 operation/task set 已稳定；
- 阶段 2 的 media envelope、subset/intersection 和显式 Target profile 已稳定；
- `/openbridge/v1/models` v2 的开放问题已决策；
- standard `/v1/models` 四字段合同保持不变。

## Direct replacement

1. `ModelExecutionInterfaces` 改为 `BTreeMap<OperationKind, OperationExecutionInterface>` 或等价 deterministic closed set。
2. 每个 interface 同时拥有：
   - downstream-safe operation contract；
   - 产生该 contract 的固定 candidate 顺序；
   - task binding；
   - Native/Bridge transform；
   - resource/continuation affinity；
   - operation-specific response budget。
3. Compiler 按 operation 生成 contribution、聚合和可达性校验，不在根模块按 Provider 名称分支。
4. Public Model 允许不同 operation 绑定不同 task；同一 operation 的 candidate 仍必须 task-compatible。
5. `/openbridge/v1/models` 直接切换 schema v2：`model_facts.tasks` + operation-keyed `interfaces`。
6. private execution contract 与 public DTO 分离；DTO 只由 compiled interface 投影。
7. OpenAPI、Swagger、examples、fixtures、Models tests 与 schema version 原子更新。

## Models v2 最小形状

```json
{
  "id": "public-model",
  "schema_version": 2,
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

要求：

- operation key 使用闭合、稳定、低基数名称；
- interface 只公开可执行事实，不公开 Provider/Target/Route/upstream model/credential；
- model facts 与 operation interface 的重复字段必须有明确语义差异；
- `supported_parameters` 只属于具体 operation interface；
- 不提供 v1 alias、双 schema 或 content negotiation shim。

## 先失败测试

- 一个 synthetic multi-task Public Model 可在不同 operation 暴露不同 task；
- 同一 operation 混合不兼容 task 启动失败；
- interface 的 candidate list 与 capability contract 同步生成；
- candidate 顺序变化不改变交集，但改变 execution order；
- operation 缺少 Native coverage 时只有显式允许的 Generation Bridge 才可补充；
- Models v2 list/retrieve 相同、schema version 固定、拓扑不泄漏；
- v1-only JSON 断言先失败，再随 direct replacement 删除。

## 删除清单

- fixed `chat_completions/responses/embeddings` execution fields；
- generation-only interface accessor 和 `has_native_candidate` 特化捷径；
- 扩展 Models v1 DTO、serializer、fixture 和测试；
- 从 DTO 反向读取 capability 的任何路径；
- schema migration alias 或 dual output。

## 退出门

- standard Models 不变；扩展 Models v2、OpenAPI、examples 和 tests 原子一致；
- preflight 仍读取 private contract；
- operation interfaces 与 candidate 顺序同源；
- topology privacy、startup validation、compiled Models、config contract 全绿；
- 完整 Rust 基线通过。

## 非目标

- 不重写 attempt engine；
- 不实现新 endpoint；
- 不从 Provider `/models` 动态产生 interface；
- 不承诺旧扩展 Models 客户端兼容。
