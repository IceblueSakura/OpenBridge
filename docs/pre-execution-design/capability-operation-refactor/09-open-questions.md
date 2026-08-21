# 09：后续决策门

本页只保留尚未进入当前焦点的未来决策。到达对应阶段时基于 live source、当前协议和真实 Provider 证据关闭，不保存已完成的决策历史。

## 1. 首个真实新 operation

阶段 5B 前选择一个 model-bound operation。必须同时具备客户端需求、当前官方 wire、明确 Provider/Target profile、失败语义和独立验收边界；
Files lifecycle、异步 resource job 与 Realtime session 不属于候选。

## 2. Models v2

只有现有 v1 无法准确表达已批准客户端合同时才切换。真实新 operation 或跨 task Public Model 只触发重新评估；确认需要 v2 后再决定
operation names、task mapping、query、缺失字段语义和 schema version 类型，并直接替换唯一公共 schema，不提供双版本、alias 或 shim。

## 3. Shared ModelIdentity

当前保持单 task executable profile。只有同一真实模型跨 task 重复注册并产生 metadata 漂移、同一 Target 确需多 task，或一个 Public Model
必须跨 operation 暴露不同 task 时，才设计共享 `ModelIdentity + TaskProfile[]`；runtime API 仍只保存一个 selected task profile。

## 4. Resource-backed identity

`file_id`、response continuation、voice/resource owner 只有在 issuer、credential owner、Target/API affinity、lifecycle、restart 与 fallback
规则完整后才能进入单独 current focus；本重构不实现 ledger。

## 5. Build-time manifest

当前继续使用 Provider-local Rust constants。只有大量 profile 文件退化为稳定纯数据并形成明确维护痛点时，才评估 checked-in manifest →
build-time typed Rust generation；不引入 runtime capability DSL 或动态注册。

## 6. Property testing

先使用确定性 table/law tests。组合规模显著增长或发现交集代数边界遗漏后，再有意引入 `proptest` 并更新锁文件。

## 7. 执行前重新核验

每个 current focus 开始前重新确认 live operation/task/schema、格式与测试基线、对应官方 wire、目标 Provider 的 source/format/limit/state
事实，以及 corpus/testkit 版本。设计包中的日期、数量和旧外部快照不能替代重新核验。