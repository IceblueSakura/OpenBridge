# 下一步目标

**用仅文本 Generation 验证 IR 化网关整体流程，再扩展 Provider 和多模态。** 产品方向见 [v2 目标](../architecture-v2/README.md#产品目标与阶段判据)，当前完成度只由[实施基线](../architecture-v2/migration.md)维护。

已有统一语义/context、显式 adapters、固定 topology、单帧 intake 和增量 response delivery，不再把这些列为待从零建立的基础。下一步应将其接到真正的 ingress/transport I/O 和消费者，而不是继续扩大公共 codec 的兼容规则集合。

## 优先顺序

| 优先级 | 建议切片 | 退出条件 |
|---|---|---|
| 1 | 最小文本 ingress 与实际交付接线 | 可信 Public Model/task 先绑定，统一 Request 经 transform/requirements、固定 candidate adapter、synthetic upstream 到真实下游 body；实际 I/O 控制背压、cancel、commit/complete |
| 2 | 选定 Agent/缓存场景与必要文本投影 | 固定 SDK 通过同一执行链完成工具续轮；按场景补 Chat content_filter、service tier/metadata 等，不要求完整 Chat API 对等 |
| 3 | 新接线后的受控 Provider 回归 | 已有[重构后 Flash 固定文本证据](../implementation-status/evidence/2026-09-29-flash-provider-adapter-acceptance.md)；随 ingress/SDK 接线变化重新选定 Provider/account/model、精确矩阵与输出边界，旧样本不证明新链或更广兼容 |
| 4 | 扩展 Provider 与多模态 | 新 wire 差异只改 adapter；真正的新能力演进共享 task/extension owner，验收 request/response/event 与资源边界 |

Chat cache-write 的双向投影和 DeepSeek 缺省零规则已属于当前合同，不再列为待建能力；兼容零不证明实际缓存写入、计费或缓存命中效果。其他 usage 细分、文本投影与上下文限制见[缺口表](../architecture-v2/migration.md#尚未映射的文本能力)。

## 紧接着建议选定的切片

**最小 ingress + caller-owned I/O**，范围由 [current-focus](current-focus.md)另行确定：

- 将现有 Public Model/task 绑定置于语义 decode 前，认证、可信 adapter/scope、最终 request 变换与固定 Route 有明确 owner。
- 将 `Attempt::push` 的单帧步进直接接到下游 readiness，不在 handler 中收完整流；严格 EOF 后才释放终态。
- 在外部可见的 I/O 边界标记 commit，成功最终交付后 complete；late error/cancel 不能变成功，提交后不 fallback。
- 首片不建凭据池、自动重试或动态插件，也不恢复整个旧运行时；可只用 synthetic upstream 与单固定候选。
- 默认离线 Rust 验收与固定 SDK gate 分开；真实 Provider/付费请求仍需单独精确授权。

## 文本端到端验证的完成门槛

```text
Client
 → authenticated, trusted Public Model/task binding
 → client adapter → semantic request/context/delivery
 → transform / requirements → fixed candidate lowering
 → provider adapter → trusted transport
 → provider response/event adapter → semantic validation
 → client response/event adapter → actual downstream I/O
 → next-turn replay
```

1. **语义与双协议**：验证所选文本/function/structured-output 场景，实际修改 IR 后最终 wire 以修改值为准。不可表示语义明确拒绝，不用静默删除换取成功。
2. **Agent 续轮**：固定 SDK 经同一执行链消费工具调用、回传 synthetic tool result 并取得最终回答；保留 call identity、Schema 顺序、replay scope 和派生 view 权威性。
3. **缓存亲和性**：合法 scope 的续轮保持约定 key/前缀/上下文；修改删除仍权威，跨来源/认证所有权不沿用 opaque state。归一化 usage 与实际计费观测分开。
4. **执行安全**：业务请求不能选择 URL/credential/profile；固定目标、body/stream 预算、取消、错误分类及实际 commit 有可验收的 I/O owner。
5. **证据分层**：独立语义/字节测试、synthetic upstream 全链与显式固定 SDK gate；真实 Provider、TLS/network、长期负载和缓存收益另行举证。

不以 hosted tools、program 执行、Codex turn 管理、state/WS、媒体或其他 task family 完整实现为前置条件。当前没有生产服务入口，不把库级验证描述为上线验收。

## 实现前需选定的边界

选定 Public Model 的承诺、固定上下游 adapter、认证/credential/scope owner，以及首片 Agent/缓存场景；再写入 current-focus。方向文档不授权真实调用、部署、提交或推送。
