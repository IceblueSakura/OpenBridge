# 06：阶段 4——Operation-first pipeline 与共享 execution

## 目标

按 operation 重组纯 request/response 语义、预检和规划，并把 stateful attempt 生命周期放入独立顶层 `execution/`。

## 依赖

- operation-indexed private registry interface 已稳定；
- Chat、Responses、Embeddings deterministic contracts 与 Models v1 全绿；
- Provider common policy、operation wire policy 与 execution lifecycle 的 owner 已明确；
- 本阶段不增加生产 endpoint。

## 目标结构

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

`pipeline/` 不执行 I/O；其中 operation driver 拥有 request preparation、response validation/rendering、JSON/SSE/binary framing、
response budget 和 retry eligibility。`execution/` 只协调 fixed candidates、credential、attempt、fallback、commit 和 cancel。

## Direct replacement

1. Generation analyzer/preflight/planner 收进 generation family；Bridge 只在该 family 内可见。
2. Embeddings 成为完整 operation family，删除独立 request wrapper 与复制 forwarding loop。
3. 提取 top-level `AttemptCoordinator`，保持 candidate order、credential rotation、cooldown、replay、commit 和 cancellation 语义。
4. Provider definition 改为 operation-owned closed dispatch；每个 operation 明确 path、headers、body hook、terminal profile 和 ceiling。
5. OpenAI-compatible 只提供共享 wire primitives，不作为未知 operation 的隐式后备。
6. Ingress 继续拥有 Axum admission、body lifecycle 和下游响应提交边界，不解释 capability 或选择 Route。

## 先失败测试

- 每个 operation 只有一个 analyzer、preflight、planner 和 response driver owner；
- non-Generation operation 无法构造 Bridge plan；
- wrong operation adapter/path/terminal 在 transport 前或响应边界失败；
- adapter 产生绝对 URI、敏感 header 或跨 Provider credential 时失败；
- 首输出后所有 operation 都禁止 retry/fallback；
- cancellation 停止 active request 与 pending backoff；
- operation-specific response budget 在 commit 前验证；
- `AttemptCoordinator` 不读取 Public DTO、task set 或 Provider 名称。

## 实施顺序

1. 提取无行为变化的 attempt state machine；
2. 迁移 Embeddings，删除旧 forwarding loop；
3. 迁移 Generation，保持 Bridge/JSON/SSE 行为；
4. 重组 pure pipeline 与 operation response driver；
5. 拆 Provider common 与 operation adapter；
6. 删除旧 wrappers、helpers、implicit fallback 和双 runtime path；
7. 运行 replay、SSE、observability、Provider boundary 与完整 Rust 基线。

## 退出门

- Chat、Responses、Embeddings wire、错误和 Models 合同保持；
- Bridge、SSE、retry/fallback/cancel、Provider boundary 与 observability tests 全绿；
- adapter 只能产生相对 URI；新 operation 有明确接入点但没有空实现；
- 完整 Rust 基线通过。

## 非目标

- 不引入 runtime operation plugin、动态 adapter discovery、新 endpoint、resource ledger 或远程控制面。
