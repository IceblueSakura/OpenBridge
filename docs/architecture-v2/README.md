# OpenBridge 语义架构

设计基线是 **Responses-first 标准语义 + scoped typed extensions**。设计描述目标与职责，不证明完整实现或生产就绪；具体行为查代码和独立测试，未闭合范围见[实施缺口](../implementation-status/generation.md)。

## 产品目标与阶段判据

目标是**多模态、Chat Completions / Responses 兼容、Agent 友好、缓存亲和性强的 IR 化网关**。

- 请求、响应和事件共享一个语义权威。同协议无旁路，跨协议按可表示性映射或明确拒绝，不以 Chat 最小交集定义 IR。
- client/Provider adapters 吸收 wire 差异，不建立厂商 IR；具名默认值不能掩盖非法值、恢复删除值或编造成功终态。
- Agent 场景保留工具身份、原始参数、结果和 reasoning 回放生命周期；协议表示不授权网关执行工具。
- 缓存亲和保持稳定前缀、工具/Schema 顺序和来源约束；不实现网关回答缓存、会话管理或跨请求粘性路由。稳定投影与真实命中收益分别判断。
- task、modality、wire 和资源生命周期分开，不能将媒体压成字符串或把独立任务强塞进 Generation。

阶段退出看选定场景的入口、IR、lowering、上游适配、响应交付与续轮能否连成可验收路径，不看字段数或测试数。推进顺序见[下一步目标](../implementation-plans/next-goal.md)，获准切片见[当前焦点](../implementation-plans/current-focus.md)。

## 处理模型

```text
Wire + trusted admission
 → Client / Provider Adapter over shared Protocol Codec
 → Task semantics + context / delivery / scoped carriers
 → Validation / Trusted Transform → Requirements
 → Fixed Candidate Representability / Lowering
 → Protocol Codec → Wire
```

纯 codec/lowering 不访问 credential、registry 或网络。运行时 I/O 与 commit 由执行边界拥有；当前模块接线见[架构](../architecture.md)。

## 架构决策

ADR 只保留有效决策、理由、后果和 owner 链接；字段细节、默认值和回归场景归代码，不在 ADR 保存历史分析或执行结果。

| ADR | 决策 |
|---|---|
| [0001](decisions/0001-semantic-core.md) | Responses-first 单一语义权威 |
| [0002](decisions/0002-task-ir-and-identities.md) | task、identity、presence 与扩展归属 |
| [0003](decisions/0003-codec-lowering-boundary.md) | codec 与目标 lowering 分离 |
| [0004](decisions/0004-capability-separation.md) | 语义、表示、执行与公共合同分离 |
| [0005](decisions/0005-execution-lifecycle.md) | 语义盲执行与显式交付生命周期 |
| [0006](decisions/0006-reasoning-ownership.md) | reasoning 与 replay 的 owner/origin/finality |
| [0007](decisions/0007-stateless-cache-affinity-and-extensions.md) | cache/session/context carrier 独立所有权 |
| [0008](decisions/0008-stable-core-and-vendor-adapters.md) | 单一 core 与显式边界 adapter |
| [0009](decisions/0009-minimal-http-text-gateway.md) | 最小认证 loopback HTTP 网关 |
| [0010](decisions/0010-canonical-model-fixed-fallback.md) | canonical model 与固定提交前 fallback |
| [0011](decisions/0011-stable-admission-provider-cache.md) | 稳定准入与 Provider-owned cache affinity |

## 设计合同与来源

- [Semantic IR](semantic-ir.md)：标准主干、attachment、身份与 presence。
- [Domain model](domain-model.md) / [capability model](capability-model.md)：task/model/profile/endpoint 与能力维度。
- [Protocol and lowering](protocol-and-lowering.md)：codec、fidelity 和目标可表示性。
- [Execution model](execution-model.md) / [invariants](invariants.md)：跨模块生命周期、安全和资源边界。
- [Rust layout](rust-layout.md)：职责划分方向，不复制文件树。
- [Responses](responses-text-profile.md) / [Chat](chat-text-profile.md) / [Schema](schema-profile.md) profiles：设计准入与刻意拒绝的范围；精确规则以 owning code 和独立预期维护。
- [固定来源](../references/upstream-sync.md)：标准/SDK 与产品私有协议的出处和许可。
- [验收方法](../references/conformance-baseline.md)：独立 decode/encode、IR 编辑、Static/Event 与失败/资源边界。执行命令归[开发指南](../development.md)，结果不保存在文档。
