# 06：阶段 4——Operation-first pipeline 与 Provider adapter

## 目标

把当前按处理阶段聚合、对 Generation/Embeddings 特化的 pipeline 和 adapter，重组为 operation-first 模块；共享层只保留真正跨 operation 的 admission、attempt、credential、retry、commit、cancellation 和 transport 生命周期。

## 依赖

- operation-indexed registry interface 已稳定；
- current Chat/Responses/Embeddings deterministic contracts 全绿；
- Provider common policy 与 operation wire policy 的边界已决策；
- 不与本阶段同时增加新生产 endpoint。

## 目标结构

```text
src/pipeline/
  generation/{analysis,preflight,planning,response}.rs
  embeddings/{analysis,preflight,planning,response}.rs
  execution/{attempt,retry,commit,cancellation}.rs

src/provider/
  definition.rs
  operation.rs
  adapter/{common,request,response}.rs

src/providers/<provider>/
  definition.rs
  adapter.rs
  media.rs
  registration.rs
```

未来 operation 只在进入当前焦点时新增自己的 pipeline 和 adapter profile。

## Direct replacement

1. Generation analyzer/preflight/planner/renderer 收进 generation operation family；Bridge 只在该 family 内可见。
2. Embeddings 迁移为完整 operation family，不再通过独立 `EmbeddingRequest` 和复制 forwarding loop 表示例外。
3. 抽取共享 `AttemptCoordinator`：
   - fixed candidate traversal；
   - credential lease/rotation；
   - retry/fallback/cooldown；
   - replay budget；
   - pre-output commit；
   - cancellation/backoff termination。
4. operation driver 仍拥有 request preparation、response validation、SSE/JSON/binary framing 和 retry eligibility。
5. Provider definition 改为 operation-owned dispatch；每个 operation 明确 path、headers、body hook、terminal profile 和 ceiling。
6. OpenAI-compatible 只提供共享 wire primitives，不再作为未知 operation 的隐式后备。

## 先失败测试

- 每个 operation 只有一个 analyzer/preflight/planner/renderer owner；
- non-Generation operation 无法构造 BridgePlan；
- wrong operation adapter/path/terminal 在 transport 前或边界失败；
- adapter 产生绝对 URI、跨 Provider header 或 credential 时失败；
- 首输出后所有 operation 都禁止 retry/fallback；
- cancellation 停止 active request 和 pending backoff；
- operation-specific response budget 在提交前验证；
- shared attempt coordinator 不读取 capability DTO，也不按 Provider 名称分支。

## 实施顺序

1. 先提取无行为变化的 attempt state machine；
2. 迁移 Embeddings 到共享 coordinator，删除旧 loop；
3. 迁移 Generation，保持 Bridge/JSON/SSE 行为；
4. 重组 pipeline 为 operation-first；
5. 拆 Provider common 与 operation adapter；
6. 迁移所有 Provider definitions；
7. 删除旧 request wrappers、forwarding helpers 和 implicit fallback；
8. 运行 process replay、SSE、observability 和 Provider boundary 基线。

## 删除清单

- 独立 `EmbeddingRequest` 和 embedding-only forwarding attempt loop；
- 全局万能 `ApiRequest` 对 non-Generation operation 的借用；
- generic OpenAI-compatible unknown-operation fallback；
- pipeline phase-first facade 中只为少数 operation 存在的平铺分支；
- Provider 名称驱动的 operation 行为；
- legacy adapter wrapper 与双 runtime path。

## 退出门

- current Chat/Responses/Embeddings wire 与错误合同保持；
- Bridge、SSE、process replay、retry/fallback/cancel、Provider boundary、observability tests 全绿；
- adapter 只能产生相对 URI；
- 新 operation 的最小接入点已明确但没有空实现；
- 完整 Rust 基线通过。

## 非目标

- 不引入 `dyn Operation` plugin；
- 不实现动态 adapter discovery；
- 不新增生产 endpoint；
- 不实现 resource ledger 或远程控制面。
