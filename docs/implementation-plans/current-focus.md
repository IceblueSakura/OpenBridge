# 当前开发焦点

## 状态

**已规划、待实施：阶段 1——Responses `reasoning.encrypted_content` 精确兼容。**

## 当前焦点

本文只授权[七阶段实施顺序](../pre-execution-design/implementation-sequence.md)中的阶段 1。阶段 2–7 仍是执行前设计，前一阶段完成不自动授权下一阶段。

## 可观察行为

所有固定 Responses Public Model 在其他字段合法时都接受：

```json
{"include":["reasoning.encrypted_content"]}
```

每个固定 candidate 按自己的真实 Upstream API contract 处理：

1. Native Upstream API 原生支持该值时，保持数组顺序和 wire 值精确转发；
2. Native Upstream API 不原生支持时，只删除已批准为 omitted-equivalent 的该元素；
3. Responses→Chat candidate 的 forwarded set 为空，由 `plan_request` 在进入 Bridge 前使用同一规则删除该 hint；
4. 删除后数组为空时删除顶层 `include`；
5. 其他 include 值继续按现有 closed contract fail closed；任何 active `include` 意外到达 Bridge converter 也必须拒绝；
6. 请求事实不筛选、跳过或重排 candidate，每个 egress body 从同一 canonical request 独立产生；
7. 不支持 candidate 不会因 hint 被跳过，fallback 时也不能继承前一 candidate 的 body mutation；
8. 删除 hint 不合成 encrypted/plaintext reasoning item，不改变 response terminal、retry/fallback 或 streaming lifecycle。

## 对应需求与设计

- [普通参数与条件输出兼容](../functional-requirements/gateway-api/parameter-compatibility.md#2-responses-include)：accepted/forwarded 边界及唯一 approved hint。
- [Models 能力合同](../functional-requirements/model-capability/model-facts-and-interface-contract.md)：`response_includes` 是公共 interface 逐值契约。
- [请求预检与路由](../functional-requirements/model-capability/request-preflight-and-routing.md)：本地拒绝、固定 candidate 与 zero egress 边界。
- [阶段 1 实施计划](../pre-execution-design/implementation-plans/01-reasoning-encrypted-content-compatibility.md)。
- [详细兼容提示设计](../pre-execution-design/responses-reasoning-encrypted-content-compatibility.md)。

## 当前基线

- `ResponseInclude` 已精确解析 `reasoning.encrypted_content`，analyzer 已处理省略、`null`、空数组和未知值。
- Public compiler 当前把每条 Route 的原生 `response_includes` 直接求交，accepted 与 forwarded 仍是同一个集合。
- `validate_interface_request` 只读取公共交集，因此 Bailian/DeepSeek 等不原生接受该值的固定 interface 仍在 Provider egress 前返回 capability 400。
- `RouteExecutionCandidate` 没有独立的 include forwarded contract。
- `plan_request` 已为每个 candidate 从 canonical request 生成独立 body，是逐值过滤的正确边界。
- Responses→Chat Bridge 当前通过 compiler 的 `reasoning.is_supported()` 门和 converter validator 显式消费 hint；ChatGPT Native 已有 exact-forward 测试。目标实现必须删除这两处重复特例：所有 Responses candidate 都由公共 accepted policy 接受，planning 在 Bridge 前完成 omission，converter 对残留 active `include` fail closed。
- 当前错误仍可能缺少字段级 `param`；全局错误模型与固定首错顺序属于阶段 2，不在本焦点内。

## 目标领域合同

实现必须保持两个闭合层次：

| 层 | 含义 | 可见性 |
|---|---|---|
| Public accepted includes | 每条固定 candidate 都能安全处理的下游请求值 | Models v1 与 preflight 可见 |
| Candidate forwarded includes | 当前 Upstream API 实际收到并原样处理的值 | 私有 execution 事实，禁止序列化 |

当前唯一 `ForwardOrOmit` 值是 `reasoning.encrypted_content`。其他 include 默认要求全部固定 candidate exact-forward；若未来需要 Bridge 转换其他值，必须另立产品合同和阶段，不能复用本 hint 的 omission policy。不得按 Provider 名称、任意字符串或整个 `include` 字段做宽泛忽略。

## 先失败的测试

实施前按顺序建立 deterministic RED：

1. **Native omit RED**：Bailian/DeepSeek 仅请求该 hint 时旧 preflight 失败；目标为接受且录制的 upstream body 不含该值。
2. **Candidate 隔离 RED**：synthetic 固定 interface 中 A 原生支持、B 仅安全省略；令 A 在收到 response headers 前发生 transport failure，B 成功，确认两个 body 独立；反转 candidate 顺序后结果仍正确。该测试只使用当前既有 fallback 边界，不引入阶段 3 的 SSE precommit 语义。
3. **Bridge owner RED**：direct converter 当前会显式消费 active hint；目标是对任何残留 active `include` fail closed，同时 Router 路径仍成功并在 Bridge 前删除 hint，两条路径都不合成 response item。
4. **Mixed negative 回归**：hint 与任一已知但不在公共 accepted set 的 include 同时出现时，整体拒绝并 zero egress，不能删除 hint 后继续发送。
5. **Typed filter 补充测试**：使用纯 typed forwarded set 验证 mixed array 只删除 approved hint，并保持其他元素、重复项和原顺序。当前生产 Provider 没有第二个可执行 include ceiling，因此该测试不是端到端 RED；不得为测试虚假提升生产 Provider 声明。首个真实第二 include 进入产品合同时再补完整 registry/transport 证明。
6. **Native forward 回归**：ChatGPT/OpenAI 等支持 candidate 精确保留该值。
7. **Models/privacy 回归**：公共 `response_includes` 包含 hint，但 Models 不含 forwarded set、Route、Provider 或 omission policy。

第 1–2、4 项必须经过真实 Router、认证、analysis、preflight、planning 和 synthetic transport；第 1–3 项必须在旧实现上按预期失败，不能先写 helper 再测试 helper 来冒充 RED。第 3 项另通过 direct converter contract 锁定 fail-closed owner。

测试归属以[阶段 1 实施计划的测试所有权表](../pre-execution-design/implementation-plans/01-reasoning-encrypted-content-compatibility.md#6-验证)为准；`provider_contract` 只保护 adapter wire 回归，不证明 candidate filtering。

## 实施顺序

1. **类型策略**：在 generation capability 内建立逐值 `NativeOnly`/`ForwardOrOmit` 闭合策略；不得加入用户可配置字符串或第三个 Bridge-only policy。
2. **Compiler 分层**：每条 Responses Route 的 accepted set 为真实 native includes 加唯一 approved hint；分别编译公共 accepted 交集与每个 `RouteExecutionCandidate` 的私有 forwarded set。Native forwarded set 只来自当前 Upstream API 的 concrete Responses profile，Responses→Chat forwarded set 为空。
3. **启动不变量**：保证 `forwarded ⊆ accepted`；任何 accepted-but-not-forwarded 值都必须是 `ForwardOrOmit`；不得修改 Provider ceiling 来满足公共 accepted contract。
4. **Preflight 保持**：preflight 只读 accepted set；未知 include 和不安全组合继续在任何 egress 前拒绝。
5. **Candidate 过滤**：在 `plan_request` 的 candidate body 构造边界、Native/Bridge 分支前按 forwarded set 逐值过滤；删除空数组顶层字段，保留其他元素的原 Value、重复项和顺序，每个 body 独立 immutable。
6. **Bridge 特例收口**：删除 compiler 中受 `reasoning.is_supported()` 约束的 include 特例和 converter 中允许该 hint 的专用 validator；把 Responses `include` 从 Bridge representable source field 收窄为 inactive-only，使任何残留 active 值 fail closed。
7. **Projection 与隐私**：Models v1 继续从 accepted set 投影；不新增 schema 字段、不泄漏 private execution contract。
8. **证据与状态**：更新 requirements、implementation status 和 tests；focused checks 后运行完整 Rust baseline 并清空 current focus。

## 明确非目标

- 不把 `ForwardOrOmit` 扩展到 `web_search_call.action.sources`、file search、image URL、logprobs 或未来 include 值。
- 不吞掉 `parallel_tool_calls`、`prompt_cache_key`、tools、reasoning level、structured output 或 output limit。
- 不实施 opaque encrypted-content replay、issuer affinity、跨 credential/Target/Provider continuation。
- 不承诺 Provider 一定返回 encrypted content，也不把 plaintext reasoning 解释为 opaque continuation。
- 不做阶段 2 的 Generation typed param/reason/validation-order 重构。
- 不修改 retry/fallback 预算、timeout、precommit、EOF、SSE terminal 或 Images 路径；candidate 隔离测试只复用现有 headers 前 transport-failure fallback。
- 不补做 Chat→Responses usage detail、`reasoning.summary`、state no-op 或其他 output normalization。
- 不升级 Models v2，不添加 compatibility shim、feature flag 或 runtime capability DSL。

## 验证顺序

先运行：

```text
cargo test --locked --test config_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
cargo test --locked --test bridge_conversion_contract
cargo test --locked --test provider_contract
```

随后运行：

```text
cargo fmt -- --check
cargo check --locked --all-targets
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

另检查 Models/OpenAPI/fixtures 和 Markdown links。只有修改 `testdata/` 或 `tools/corpus/` 时才追加 Python/corpus baseline。真实 Provider、Hermes runtime、负载与长期运行不属于首轮完成门；若执行必须单独记录有限证据。

## 完成判定与回滚

完成必须同时满足：Native omit、candidate 隔离和 Bridge owner RED 转绿；mixed negative 与 Native exact-forward 回归保持；Router 在 Bridge 前完成 omission 且 direct Bridge 对残留 active `include` fail closed；Models 只暴露 accepted contract；所有本地拒绝 zero egress；requirements/status 与 live source 一致；current focus 恢复为空。纯 typed mixed 保序测试是补充证明，不代替首个真实第二 include 未来所需的端到端 contract。

回滚单位是 accepted/forwarded 类型、compiler、planning 过滤、测试和文档的完整阶段 commit。不得只恢复 preflight 拒绝、留下虚假 Models 声明，也不得只回滚过滤而保留宽松 accepted set。
