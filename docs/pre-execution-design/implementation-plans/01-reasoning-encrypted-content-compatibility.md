# 阶段 1：`reasoning.encrypted_content` 精确兼容

> **状态：候选实施计划，本文自身不构成实施授权。** 本计划已被选择为下一项 current focus，但只有
> [`current-focus.md`](../../implementation-plans/current-focus.md) 表示当前实施授权。详细语义由
> [兼容提示设计](../responses-reasoning-encrypted-content-compatibility.md)与正式功能需求拥有。

## 1. 可观察结果

所有固定 Responses Public Model 都安全接受 `include: ["reasoning.encrypted_content"]`：

- candidate 的 Upstream API 原生支持该值时，按原顺序精确保留；
- candidate 不原生支持、但该精确 hint 已被批准为 omitted-equivalent 时，只删除该元素；
- 删除后数组为空时删除顶层 `include`；
- 其他 `include` 值仍按现有闭合能力合同 fail closed；
- 请求事实不筛选、跳过或重排 Route，commit 后不 fallback，也不合成任何 response item。

## 2. 已验证基线与 owner

- `src/core/capability/generation.rs::ResponseInclude` 已闭合建模该 wire 值。
- `src/pipeline/generation/analysis.rs` 已解析省略、`null`、空数组和逐值 include。
- `src/pipeline/generation/preflight.rs::validate_interface_request` 当前只接受 Public interface 的
  `response_includes`，因此仍会拒绝 Bailian/DeepSeek 路径。
- `src/registry/public_model/compiler/contribution.rs::protocol_specific_capabilities` 当前把 Route 原生转发集合直接贡献为公共接受集合。
- `src/registry/public_model/execution.rs::RouteExecutionCandidate` 已冻结 candidate 执行事实，但尚未拥有独立的 include 转发策略。
- `src/pipeline/generation/planning.rs::plan_request` 已按 candidate 从同一 canonical request 构建 immutable body，是逐 candidate 过滤的唯一正确边界。
- Bridge 当前已有 compiler + converter 的显式消费特例；目标实现将其替换为 planning-owned omission，并让 converter 对残留 active `include` fail closed。Native ChatGPT 已有 exact-forward 测试。

## 3. 先失败的测试

在改实现前建立以下 RED：

1. Bailian/DeepSeek Responses Public Model 仅请求该 hint 时，旧 preflight 返回 capability 400；目标是接受且 transport body 不含该值。
2. 一个 synthetic 固定 interface 中 candidate A 原生支持、candidate B 只安全省略；A 在收到 response headers 前发生 transport failure 后 B 成功，A/B 收到互不污染的不同 body；反转 candidate 顺序后仍正确。该测试不引入阶段 3 的 SSE precommit 语义。
3. direct converter 当前会显式消费 active hint；目标是对任何残留 active `include` fail closed，同时 Router 在 Responses→Chat Bridge 前删除 hint 并继续成功。
4. hint 与任一已知但未进入公共 accepted set 的 include 混合时，整体拒绝并 zero egress；这是既有 fail-closed 回归，不冒充 RED。
5. 纯 typed filter 补充测试验证：若 forwarded set 保留同数组中的其他值，只删除 approved hint，并保持原 Value、重复项和顺序。当前生产 Provider 没有第二个可执行 include ceiling，该测试不冒充端到端 RED，也不得通过提升生产 ceiling 制造场景。
6. 支持 candidate 继续精确保留 hint；Models `response_includes` 表示公共 accepted set，但不泄漏 candidate forwarded set、Route 或 omission policy。

第 1–2、4 项必须经过真实 Router、认证、analysis、preflight、planning 和 synthetic transport；第 1–3 项必须在旧代码上按预期失败。第 3 项另由 direct converter contract 锁定 fail-closed owner；不能用直接测试新 helper 代替第 1–2 项。

## 4. 实施步骤

1. 在 generation capability 内引入闭合、类型化的逐值处理策略，区分 `NativeOnly` 与当前唯一 approved `ForwardOrOmit`；不得使用任意字符串、Provider 名称分支或 Bridge-only 第三状态。
2. 分离 Public accepted includes 与 candidate forwarded includes：
   - Public accepted set 表示每条固定 Route 都能安全处理；
   - Native forwarded set 仍来自该 Upstream API 的真实 concrete Responses profile；
   - Responses→Chat candidate 的 forwarded set 为空，approved hint 仍进入 Route accepted set。
3. 在 registry compiler 中同时编译公共 accepted contract 与每个 `RouteExecutionCandidate` 的私有 forwarded contract；启动校验保证 `forwarded ⊆ accepted`，并拒绝未知策略、越过 Provider ceiling 或非 approved 的 accepted-but-not-forwarded 值。
4. 保持 preflight 只读取公共 accepted set；省略、`null`、空数组继续归一为无请求，未知值继续拒绝。
5. 在 `plan_request` 的 candidate body 构造阶段、Native/Bridge 分支前逐值过滤 approved hint；保留其他元素的原 Value、重复项和顺序。每个 body 从 canonical request 独立派生，禁止共享可变 JSON。
6. 删除 compiler 中 `reasoning.is_supported()` 控制的 Bridge include 特例和 converter 专用 include validator；将 Responses `include` 收窄为 Bridge inactive-only source field，使任何残留 active include fail closed。不保留双路径、legacy alias 或按 Provider 名称的兼容 shim。
7. 保持 Models v1 schema；`response_includes` 与 `supported_parameters` 从同一 accepted contract 投影，私有 forwarded contract 不序列化。
8. 更新功能需求、相关 status、测试和必要 fixture；不得把候选计划文字写成实现事实。

## 5. 非目标

- 不泛化 `ForwardOrOmit` 到其他 include 值或普通参数。
- 不修改 `parallel_tool_calls`、`prompt_cache_key`、tools、reasoning level 或 output limit。
- 不实现 opaque encrypted-content replay、issuer affinity 或跨 Provider continuation。
- 不承诺 Provider 一定返回 encrypted item，不合成 plaintext/opaque reasoning。
- 不执行阶段 2 的 Generation 全局 `param`/reason/首错重构。
- 不改变 retry、fallback、stream terminal、EOF 或 timeout policy。
- 不补做 usage detail、`reasoning.summary`、state no-op 或其他字段兼容/输出归一化。

## 6. 验证

测试所有权：

| 合同 | 主要 owner |
|---|---|
| Provider ceiling、Target subset 与启动不变量 | `tests/config_contract.rs` |
| Public accepted set、Models projection 与 topology privacy | `tests/forwarding_contract/models.rs` |
| Native omit、mixed negative 回归、zero egress 与 candidate body 隔离 | `tests/forwarding_contract/{admission,native}.rs` |
| Router-owned Bridge omission 与 direct converter fail-closed | `tests/bridge_forwarding_contract.rs`、`tests/bridge_conversion_contract.rs` |
| Provider path/model rewrite 与既有 native wire 回归 | `tests/provider_contract.rs`；不能用它代替 planning/candidate isolation 证明 |

真实 Provider 不作为 RED；上述行为使用 synthetic transport 经过真实 ingress/pipeline 边界。纯 typed mixed 保序测试放在过滤 helper 的就近单元测试中，并明确只证明逐值算法。

Focused：

```text
cargo test --locked --test config_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
cargo test --locked --test bridge_conversion_contract
cargo test --locked --test provider_contract
```

完整 Rust 基线：

```text
cargo fmt -- --check
cargo check --locked --all-targets
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

另检查 Models/OpenAPI/Markdown links。真实 Provider、Hermes runtime、负载与长期运行属于更高层独立证据，不是首轮完成门。

## 7. 退出与回滚

完成必须同时满足：Native omit、candidate 隔离和 Bridge owner RED 转绿；mixed negative、其他 include fail-closed 与 Native exact-forward 回归保持；Models 不泄漏 topology；Router-owned Bridge omission 与 direct Bridge fail-closed 通过；status 更新且 current focus 清空。纯 typed mixed 保序测试不替代未来第二个真实 include 的端到端证明。

回滚单位是 accepted/forwarded 类型、compiler、planning、tests 和文档的完整阶段 commit。不得只回滚过滤、留下虚假的公共接受声明。
