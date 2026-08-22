# 09：后续决策门

本页只保留尚未进入当前焦点的未来决策。到达对应阶段时基于 live source、当前协议和真实 Provider 证据关闭，不保存已完成的决策历史。

每项标注当前状态：**保留**表示仍待决策或每次执行必做；**关闭（条件触发）**表示当前明确不采用，仅在触发条件出现时才重新打开。

## 1. Models v2

> **状态：关闭（条件触发）** —— 仅当 v1 无法准确表达已批准客户端合同时重新打开。

只有现有 v1 无法准确表达已批准客户端合同时才切换。真实新 operation 或跨 task Public Model 只触发重新评估；确认需要 v2 后再决定
operation names、task mapping、query、缺失字段语义和 schema version 类型，并直接替换唯一公共 schema，不提供双版本、alias 或 shim。

## 2. Shared ModelIdentity

> **状态：关闭（条件触发）** —— 仅当出现跨 task 重复注册、metadata 漂移或确需多 task 的证据时重新打开。

当前保持单 task executable profile。只有同一真实模型跨 task 重复注册并产生 metadata 漂移、同一 Target 确需多 task，或一个 Public Model
必须跨 operation 暴露不同 task 时，才设计共享 `ModelIdentity + TaskProfile[]`；runtime API 仍只保存一个 selected task profile。

## 3. Resource-backed identity

> **状态：关闭（条件触发）** —— 本重构不实现 ledger；仅当 issuer/affinity/lifecycle 规则完整后另立 current focus。

`file_id`、response continuation、voice/resource owner 只有在 issuer、credential owner、Target/API affinity、lifecycle、restart 与 fallback
规则完整后才能进入单独 current focus；本重构不实现 ledger。

## 4. Build-time manifest

> **状态：关闭（条件触发）** —— 当前继续使用 Rust constants；仅当 profile 退化为稳定纯数据并形成维护痛点时重新打开。

当前继续使用 Provider-local Rust constants。只有大量 profile 文件退化为稳定纯数据并形成明确维护痛点时，才评估 checked-in manifest →
build-time typed Rust generation；不引入 runtime capability DSL 或动态注册。

## 5. Property testing

> **状态：关闭（条件触发）** —— 当前使用确定性 table/law tests；仅当组合规模增长或发现交集代数遗漏时重新打开。

先使用确定性 table/law tests。组合规模显著增长或发现交集代数边界遗漏后，再有意引入 `proptest` 并更新锁文件。

## 6. 执行前重新核验

> **状态：保留** —— 每个 current focus 开始前的必做流程，不是一次性决策。

每个 current focus 开始前重新确认 live operation/task/schema、格式与测试基线、对应官方 wire、目标 Provider 的 source/format/limit/state
事实，以及 corpus/testkit 版本。设计包中的日期、数量和旧外部快照不能替代重新核验。
