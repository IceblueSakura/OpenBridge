# 当前架构

OpenBridge 将共享语义库接入最小认证 loopback Generation 网关。这里说明跨模块职责；字段映射、默认值、预算和失败分支由 owning code、注释与独立测试维护。设计理由见 [ADRs](architecture-v2/README.md#架构决策)，未闭合范围见[实施缺口](implementation-status/generation.md)。

```text
Authenticated bounded HTTP input
 → fixed Public Model/task lookup
 → Client Adapter.decode_request
 → task semantics + context/delivery + scoped carriers/fidelity
 → trusted transform / public admission / requirements
 → fixed candidate projection → prepared request/auth → HTTP transport
 → Provider Adapter JSON/SSE decode → Response/Event IR
 → Client Adapter projection → downstream body handoff / commit / complete
```

## 模块所有权

| Owner | 责任 |
|---|---|
| [semantic](../src/semantic/mod.rs) | task 类型、验证、requirements、reducer；标准 context/delivery，不依赖 protocol/provider/execution |
| [adapter](../src/adapter/mod.rs) | 可信双向 profile 选择、统一 request 与目标投影；不访问网络或凭据 |
| [protocol](../src/protocol/mod.rs) | 共用语法、envelope/event codecs、strict JSON、具名适配和来源保真 |
| [lowering](../src/lowering/generation.rs) | 最终不可变语义的目标可表示性；不选择 Provider、不恢复删除值 |
| [provider](../src/provider/mod.rs) | 可信 origin/path、认证材料和 HTTP 错误分类 |
| [credential](../src/credential/mod.rs) | profile-neutral manager、显式授权 drivers 与逐账户文件 store；管理生命周期和认证 I/O，不接入 Gateway 数据面 |
| [topology](../src/topology/mod.rs) | canonical/public model、Route、Endpoint 的固定关系与编译 |
| [execution](../src/execution/mod.rs) | 候选计划、请求准备、增量 intake、响应投影和显式交付生命周期 |
| [SSE transport](../src/transport/sse.rs) / [HTTP transport](../src/transport/http.rs) | 有界 framing / 对已准备可信请求执行 I/O；不解释或改写 IR |
| [gateway](../src/gateway/mod.rs) | 认证、启动准入、预算和实际 HTTP body 所有权 |
| [binary](../src/bin/openbridge.rs) | 显式环境变量 bootstrap 与 loopback listener |

## 容易混淆的边界

- **注册不等于启用**：catalog 声明与 bootstrap 激活分开。Public Model 的语义准入也不是响应 reported facts 的白名单。现场查询方法见 [AGENTS](../AGENTS.md#current-provider-model-and-compatibility-information)。
- **适配不等于第二份语义**：[WireRules](../src/protocol/adaptation.rs) 声明受信规则，[fidelity](../src/protocol/fidelity.rs) 只保存有界来源/依赖记录。它们不能覆盖最终 IR。
- **策略不拥有 I/O**：[selector](../src/execution/plan.rs) 与 [fallback policy](../src/execution/fallback.rs) 是纯策略；[exchange](../src/gateway/exchange.rs) 协调候选/预算，[intake](../src/gateway/intake.rs) 拥有一个上游 body 的解码/投影，[body](../src/gateway/body.rs) 拥有 publication、acknowledgement 和取消。
- **编码、发布、提交、完成不同**：发布先冻结候选前移，body handoff 确认 commit，严格上游关闭与最终 handoff 决定完成；后续失败只能中止，不能伪造成功或拼接另一 attempt。具体状态转换和竞态处理留在对应实现。
- **缓存不是会话管理**：[cache projection](../src/protocol/cache.rs) 投影显式 Provider carrier；标准 identity hints 与 cache hints 独立。无网关回答缓存、负载均衡或跨请求粘性路由。
- **探测不是产品合同**：[probe tooling](../examples/probe_support/) 拥有计划、共享预算与脱敏结果，[diagnostics](../src/gateway/diagnostics.rs) 提供有界元数据。工具可选项、执行历史和 sink 状态不能决定产品准入。

启动和公共 HTTP 用法见[网关指南](http-gateway.md)，检查命令见[开发指南](development.md)，真实调用的授权与预算见[Probe 指南](probes.md)。
