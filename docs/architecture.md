# 当前架构

当前 crate 是 v2 Rust 库：统一语义核心、显式边界 adapters、纯目标 lowering、固定 topology 和 caller-driven execution。`gateway` 和 `transport::http` 已将其接成最小 loopback Generation 服务（文本输出及选定 user 图片输入 slice）；没有凭据池或生产级运行保障。旧运行时见 [Git 归档](archive.md)；设计合同见 [v2 架构](architecture-v2/README.md)。

```text
Authenticated, bounded HTTP request
 → fixed Public Model/task lookup (strict envelope identity only)
 → Client Adapter.decode_request
 → adapter::Request
     task semantics + semantic context/delivery + scoped carriers/fidelity
 → trusted output-budget transform / public admission / requirements
 → fixed endpoint Adapter.encode_request (lowering + context projection)
 → prepared target/auth/body → caller-owned transport
 → Attempt: Provider Adapter JSON/SSE decode
 → Response/Event IR
 → ResponseDelivery: Client Adapter lowering/encode
 → caller-owned downstream I/O → explicit commit / complete
```

| Owner | 当前责任 |
|---|---|
| `src/semantic/` | Generation ordered items、settings、usage、独立语义合同、validation、requirements、reducer；`context.rs` 拥有标准 cache/execution hints、reported context 和 delivery intent，不依赖 protocol/provider/execution |
| `src/adapter/` | 双向边界 facade；`Dialect` 将 Standard/OpenBridge 与固定 Provider profiles 组合为显式 wire rules；统一 request 表示与纯目标 context 投影，不访问网络/凭据/registry |
| `src/protocol/` | 共用 Chat/Responses 语法、完整 envelope、event codecs、strict JSON/SSE adapter；`adaptation.rs` 执行可信规则，`fidelity.rs` 保管有界来源/依赖记录 |
| `src/lowering/` | 对不可变最终语义检查固定目标可表示性，构造 codec 输入；不选择 Provider，不恢复删除值 |
| `src/provider/` | 可信 origin/路径、认证材料边界、HTTP 错误分类；请求 Debug 不打印 auth 或 body |
| `src/topology/` | 固定 Model/Route/Endpoint 编译；`catalog/models.rs` 显式注册 canonical，`catalog/bindings.rs` 声明公开/上游别名、profile 与激活绑定，facade 统一构造。表示合同内嵌独立语义词汇；不持有秘密 |
| `src/execution/` | 固定 plan、请求准备、单帧增量 intake、响应投影和单调交付生命周期；I/O 调用方确认 commit/complete |
| `src/transport/sse.rs` | 有界 SSE framing，无语义判断或 socket 所有权 |
| `src/transport/http.rs` | 对已准备请求执行 HTTP；不继承入站 headers、不跟随重定向或隐式重试，不解释 IR |
| `src/gateway/` | `config` 在启动时绑定 entry/credential/scope；`admission` 拥有 body 与可信输出预算；`http` 拥有认证入口和 shutdown；`exchange` 协调候选/预算，`intake` 拥有一次上游 decode/projection，`body` 拥有实际交付 acknowledgement/取消；`diagnostics` 为 opt-in、认证后的有界元数据 writer，不进入 IR 或业务响应 |
| `src/bin/openbridge.rs` | 显式环境变量 bootstrap 与 loopback listener；不读取旧私有配置 |

## 适配与保真

厂商差异不形成另一套 Generation/Usage。兼容默认值按 [ADR 0008](architecture-v2/decisions/0008-stable-core-and-vendor-adapters.md)由具名规则显式选择并记录来源，不能覆盖 reported 值或掩盖非法值；当前选择与理由由 `src/adapter/`、`src/protocol/adaptation.rs` 及其 owning codec 注释维护。

`topology::CanonicalModel` 独立拥有模型 identity/task/语义支持；Public Model 引用 canonical identity 并可公开更窄的语义合同，Endpoint 的 upstream spelling、Provider/profile 与 typed 图片约束不改写模型支持。编译拒绝未知/跨模型绑定与超出 canonical 支持的公开语义。图片目标约束与协议 profile 相交；全局安全预算不是上游模型额度。

Public Model 的语义合同和 Endpoint 的表示合同约束请求准入及目标投影，不是客户端响应格式的报告事实白名单。`gateway::config` 独立构造客户端 wire 表示合同，保留 fixed reported-fact policy 与受信 replay scope；不能因为模型不允许请求某控制字段而删掉或拒绝响应中已报告的对应事实。纯 codec/lowering 仍负责未实现语义和目标协议无槽位的拒绝。

标准 response context 在 semantic 中，instruction echo 的 wire fidelity 独立保存。classified extras 绑定协议、适配合同、可信来源和响应依赖；仅终态捕获/输出，目标不兼容或语义修改后不恢复旧值。encrypted replay 继续使用其更严格的 owner/origin/finality 合同。

厂商 routing/billing facts 同样受有界来源/响应依赖约束；重复 view 和特殊流式形式只在显式规则下验证。当前映射查询实现与独立回归，不另维护厂商适配表或测试结果页。

## 流式与执行边界

`Attempt::push` 最多消费一个 frame，返回已验证语义 events；不保存整个 event log。`ResponseDelivery::encode_events` 增量投影，I/O caller 保留未消费后缀并控制背压。Attempt 只暂存终态，严格 EOF 验证成功后，`finish_stream` 才编码成功/非成功终态。输出字节不等于实际提交：调用方在外部可见边界调用 `commit`，最终交付后调用 `complete`。late failure/cancel 不得恢复为成功或 post-commit fallback。

最小 HTTP body worker 等待每个输出 frame 被 body poll 交给 server transport 后才确认 commit；完成所有 handoff 后才 complete，不声称客户端已收到。独立绝对 deadline 在 body 不被消费时仍释放上游，drop/shutdown 同样取消。未发布下游 frame 的失败按显式 Route policy 决定是否前移；终止时返回脱敏 JSON 错误。frame 已发布或 HTTP response 已交出后的错误中止 body，不能更换候选、换状态或合成成功终态。具体入口、预算与启动合同见 [HTTP 网关指南](http-gateway.md)和 [ADR 0009](architecture-v2/decisions/0009-minimal-http-text-gateway.md)。

入口可显式激活多个固定 Route 成员，启动按编译顺序绑定独立 Endpoint/credential/scope。完整最终请求按 RequireAll 或 SkipUnrepresentable 策略独立预检；默认只执行首个兼容成员；显式 BeforeCommit Route 允许固定顺序、有界前移，不重试同成员、不并行竞速。`execution::plan::select_candidates` 是 library/Gateway 共用的唯一有界候选选择器，保留封闭拒绝类别；`execution::fallback` 只决定资格。exchange/intake/body 分工执行，整条链保持单个 permit/绝对 deadline/Trace，各候选新建 Attempt/ResponseDelivery。下游 frame 向 HTTP owner 发布后冻结候选，body handoff 仍是独立 commit 边界。默认 bootstrap 保持单成员；多成员入口的 opaque continuation 暂不准入，不能用模型相同扩大 issuer 权限。Provider 的 native Responses 路径可缺省；topology 编译会拒绝为未声明协议创建 Endpoint。各 Provider/模型的当前绑定及实例启用情况按 [AGENTS 查询方法](../AGENTS.md#current-provider-model-and-compatibility-information)核对，不从名称、目录或历史成功推断。`tests/transport/chain.rs` 验证 library execution；`tests/gateway.rs` 经过真实 Router 和 synthetic HTTP Provider；固定 SDK 同时保留 codec fixture gates 与经过同一 Gateway 的独立全链 gate。`examples/live_probe.rs` 仍是另需授权的库级诊断入口，旧 live 结果不证明新服务入口的外部兼容。

## 稳定接入与 Provider 缓存

Public Model 合同不随新增上游静默变化。模型语义、客户端 context 准入、目标 identity/cache 投影分别检查。`protocol::cache` 声明 cache key/retention/options/session carrier；`semantic::context::ClientIdentityHints` 独立于 `CacheHints`，不因缓存 hint 省略而丢失。`session_id` 是 OpenBridge scoped body 扩展，目标不支持时拒绝；不从 user/cache key/replay scope 推导，不透传入站 session headers。

缓存友好仅指稳定投影/前缀及 Provider 自带功能。没有网关负载均衡、回答缓存、会话管理或粘性路由。跨请求仍按固定 Route 顺序；不维护会话落点。源字段和目标投影的边界见 [ADR 0011](architecture-v2/decisions/0011-stable-admission-provider-cache.md)，实时接受和命中收益必须独立验证。

## 探测执行边界

`examples/probe_support/` 收敛 live SDK 入口的计划、跨进程 SQLite 预算、owned listener、raw wire 观察、场景 oracle 和白名单结果。Rust 库级与仓库内 pi 执行器共享同一 run/attempt，不各自重置额度；本机 pi 配置/凭据仍不入仓。Gateway 的可选诊断 writer 通过关联 ID 提供原调用的状态/最后阶段；队列/文件有界，失败不影响业务响应，无 payload 日志或重试策略。操作合同由 [probe 指南](probes.md)维护。

## 验证入口

- `tests/semantic.rs`：独立语义/codec/profile/变换反例。
- `tests/transport.rs`：framing、协议终态、增量执行/显式 commit 与 synthetic body I/O。
- `tests/gateway.rs`：真实 Router 全链 smoke 与隔离环境 binary 启动边界。
- `tests/sdk_loopback.rs`：显式 ignored 的固定 SDK codec / gateway gates。

具体命令与外部验收边界见[开发指南](development.md)，当前缺口只由[当前能力与边界](implementation-status/generation.md)维护。
