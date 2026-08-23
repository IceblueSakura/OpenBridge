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
- Bridge 已有显式消费该 hint 的先例；Native ChatGPT 已有 exact-forward 测试。

## 3. 先失败的测试

在改实现前建立以下 RED：

1. Bailian/DeepSeek Responses Public Model 仅请求该 hint 时，旧 preflight 返回 capability 400；目标是接受且 transport body 不含该值。
2. 一个 synthetic 固定 Route 中 candidate A 原生支持、candidate B 只安全省略；首 candidate precommit 失败后，A/B 收到互不污染的不同 body。
3. mixed include 数组只删除 approved hint，原生支持值保持原顺序；未获批准值仍零 egress。
4. 支持 candidate 继续精确保留 hint；现有 ChatGPT Native 与 Responses→Chat Bridge 测试保持。
5. Models `response_includes` 表示公共 accepted set，但不泄漏 candidate forwarded set、Route 或 omission policy。

RED 必须证明旧代码的确定性本地阻断或 candidate 过滤缺失，不能用直接测试新 helper 代替。

## 4. 实施步骤

1. 在 generation capability 内引入闭合、类型化的逐值处理策略，至少区分 `Forward` 与当前唯一 approved `ForwardOrOmit`；不得使用任意字符串或 Provider 名称分支。
2. 分离 Public accepted includes 与 candidate forwarded includes：
   - Public accepted set 表示每条固定 Route 都能安全处理；
   - Native forwarded set 仍来自该 Upstream API 的真实 include ceiling；
   - Bridge 只贡献 converter 明确消费的值。
3. 在 registry compiler 中同时编译公共 accepted contract 与每个 `RouteExecutionCandidate` 的私有 forwarded contract；启动校验拒绝未知策略、越过 Provider ceiling 或无法消费的值。
4. 保持 preflight 只读取公共 accepted set；省略、`null`、空数组继续归一为无请求，未知值继续拒绝。
5. 在 `plan_request` 的 candidate body 构造阶段逐值过滤 approved hint。每个 body 从 canonical request 独立派生，禁止共享可变 JSON。
6. 删除被新统一策略取代的 Bridge/Provider 特例；不保留双路径、legacy alias 或按 Provider 名称的兼容 shim。
7. 保持 Models v1 schema；`response_includes` 与 `supported_parameters` 从同一 accepted contract 投影，私有 forwarded contract 不序列化。
8. 更新功能需求、相关 status、测试和必要 fixture；不得把候选计划文字写成实现事实。

## 5. 非目标

- 不泛化 `ForwardOrOmit` 到其他 include 值或普通参数。
- 不修改 `parallel_tool_calls`、`prompt_cache_key`、tools、reasoning level 或 output limit。
- 不实现 opaque encrypted-content replay、issuer affinity 或跨 Provider continuation。
- 不承诺 Provider 一定返回 encrypted item，不合成 plaintext/opaque reasoning。
- 不执行阶段 2 的 Generation 全局 `param`/reason/首错重构。
- 不改变 retry、fallback、stream terminal、EOF 或 timeout policy。

## 6. 验证

Focused：

```text
cargo test --locked --test config_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
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

完成必须同时满足：RED 转绿；candidate body 隔离成立；其他 include 保持 fail closed；Models 不泄漏 topology；Bridge 与 Native 回归通过；status 更新且 current focus 清空。

回滚单位是 accepted/forwarded 类型、compiler、planning、tests 和文档的完整阶段 commit。不得只回滚过滤、留下虚假的公共接受声明。
