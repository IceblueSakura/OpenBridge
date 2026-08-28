# Semantic evaluation methods

## Metadata

| Field | Value |
|---|---|
| Source snapshot | RULER `c3f5e3b4f87f97e048793bb510a3a6b19a46bf3a`; NoLiMa `cb14780b249fecf2851127b2101a062c1b2c6430`; LongMemEval `9e0b455f4ef0e2ab8f2e582289761153549043fc`; BFCL V4 published checkpoint `f7cf7359b7ac615a0b294831c5ba2bc95ee4a000`; ToolSandbox `165848b9a78cead7ca7fe7c89c688b58e6501219`; JSONSchemaBench `8003e8405c4d8d8b327b1eb472c9856297d75493`; LongBench v2 ACL 2025, DOI `10.18653/v1/2025.acl-long.183` |
| Last reverified | 2026-08-28：官方仓库、论文页和 BFCL leaderboard |
| Scope | 长上下文 retrieval/integration/update、function-tool、stateful tool use、strict structured output 的任务设计与证据边界 |
| Evidence boundary | 不证明 OpenBridge 当前实现、任何 Provider/model 的真实质量、API entitlement、tokenizer、费用、负载或长期稳定性 |
| Recheck trigger | 引入外部数据、live runner、model-quality gate、长上下文产品承诺，或上游 benchmark/schema/license 变化 |

## 1. 研究结论

### 长上下文

- [RULER](https://github.com/NVIDIA/RULER/tree/c3f5e3b4f87f97e048793bb510a3a6b19a46bf3a)提供可配置长度与复杂度的 synthetic retrieval、multi-hop tracing、aggregation 和 QA；它明确把简单 NIAH 与更复杂任务区分，并说明 synthetic tasks 不能替代 realistic tasks。可借鉴“同一任务跨长度与位置重复”的结构，但不能把其排行榜替代目标系统证据。
- [NoLiMa](https://github.com/adobe-research/NoLiMa/tree/cb14780b249fecf2851127b2101a062c1b2c6430)要求问题与 needle 最小化字面重叠，并以短上下文基线的 85% 定义其论文中的 effective length。其代码和 needle data 受禁止商业使用的 Adobe Research License 约束；商业或产品仓库只能采用抽象任务思想，不能直接复制 payload。
- [LongMemEval](https://github.com/xiaowu0162/LongMemEval/tree/9e0b455f4ef0e2ab8f2e582289761153549043fc)覆盖 information extraction、multi-session reasoning、knowledge updates、temporal reasoning 和 abstention。knowledge update 可用于设计旧事实与 superseding fact 的冲突任务，但第三方对话来源仍需单独审计。
- [LongBench v2](https://aclanthology.org/2025.acl-long.183/)覆盖真实单/多文档、长对话、代码库和结构化数据任务，说明 realistic long-context reasoning 不能由 synthetic retrieval 一项替代；完整数据不适合作为无来源审计的默认仓库 fixture。

### Gateway 与工具语义

- [BFCL V4](https://gorilla.cs.berkeley.edu/leaderboard.html)把 function calling 扩展到 single-turn、multi-turn、memory、web search、hallucination 和 format sensitivity。gateway contract 测试可借鉴 function name、JSON arguments、call set/order 和 tool-result grounding，但 leaderboard 分数不等于 gateway correctness。
- [ToolSandbox](https://github.com/apple/ToolSandbox/tree/165848b9a78cead7ca7fe7c89c688b58e6501219)以 stateful world、implicit dependency、on-policy conversation 和 intermediate/final milestones 评估 agent tool use。没有完整 Agent loop 或 stateful tool executor 的系统不应把该类结果列为 required oracle。
- [JSONSchemaBench](https://github.com/epfl-dlab/JSONSchemaBench/tree/8003e8405c4d8d8b327b1eb472c9856297d75493)主要衡量 structured-output generation/engine 的 JSON Schema coverage、效率和质量影响；其集合来自多个上游 schema 来源，导入前需要逐项审计 artifact 与许可证。

## 2. 中性采用边界

这些外部方法评估的对象并不相同：

1. gateway wire、conversion 和 transport correctness 需要确定性协议 fixture；
2. function/structured/context 结果需要独立 semantic oracle；
3. mock/testkit loopback 只证明相应 mock 与 adapter 路径；只有显式接入 SUT 的 loopback 才可能证明对应 gateway path；
4. live model benchmark 证明固定运行条件下的模型/系统结果，不自动证明 gateway contract；
5. leaderboard、字段接受、一次成功和长期稳定性不能互相替代。

采用时应保留任务、版本、长度、位置、reasoning 设置、scorer 和执行环境，且避免把一个 benchmark 的阈值提升为其他系统的产品 SLO。

## 3. License 与再分发边界

NoLiMa 明确限制商业使用；LongBench v2、LongMemEval 和 JSONSchemaBench 又组合或整理了其他来源。即使顶层代码有开源许可证，也不能推定全部数据 payload 可直接再分发。最安全的默认采用方式是只借鉴任务结构并自主编写 synthetic case；任何外部样本导入都应固定具体 artifact、版本、license、attribution 和允许的再分发范围。
