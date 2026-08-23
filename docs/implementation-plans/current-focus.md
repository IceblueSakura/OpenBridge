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

每个固定 candidate 按自己的真实 Upstream API contract处理：

1. 原生支持该值时，保持数组顺序和 wire 值精确转发；
2. 不原生支持、但该精确 hint 已批准为 omitted-equivalent 时，只删除该元素；
3. 删除后数组为空时删除顶层 `include`；
4. 其他 include 值继续按现有 closed contract fail closed；
5. 请求事实不筛选、跳过或重排 candidate，每个 egress body 从同一 canonical request独立产生；
6. 不支持 candidate 不会因 hint 被跳过，fallback时也不能继承前一 candidate的body mutation；
7. 删除 hint 不合成 encrypted/plaintext reasoning item，不改变response terminal、retry/fallback或streaming lifecycle。

## 对应需求与设计

- [普通参数与条件输出兼容](../functional-requirements/gateway-api/parameter-compatibility.md#2-responses-include)：accepted/forwarded边界及唯一approved hint。
- [Models能力合同](../functional-requirements/model-capability/model-facts-and-interface-contract.md)：`response_includes`是公共interface逐值契约。
- [请求预检与路由](../functional-requirements/model-capability/request-preflight-and-routing.md)：本地拒绝、固定candidate与zero egress边界。
- [阶段1实施计划](../pre-execution-design/implementation-plans/01-reasoning-encrypted-content-compatibility.md)。
- [详细兼容提示设计](../pre-execution-design/responses-reasoning-encrypted-content-compatibility.md)。

## 当前基线

- `ResponseInclude`已精确解析`reasoning.encrypted_content`，analyzer已处理省略、`null`、空数组和未知值。
- Public compiler当前把每条Route的原生`response_includes`直接求交，accepted与forwarded仍是同一个集合。
- `validate_interface_request`只读取公共交集，因此Bailian/DeepSeek等不原生接受该值的固定interface仍在Provider egress前返回capability 400。
- `RouteExecutionCandidate`没有独立的include forwarded contract。
- `plan_request`已为每个candidate从canonical request生成独立body，是逐值过滤的正确边界。
- Responses→Chat Bridge已有显式消费hint的特例；ChatGPT Native已有exact-forward测试。新统一策略必须替代重复特例而非并存。
- 当前错误仍可能缺少字段级`param`；全局错误模型与固定首错顺序属于阶段2，不在本焦点内。

## 目标领域合同

实现必须保持两个闭合层次：

| 层 | 含义 | 可见性 |
|---|---|---|
| Public accepted includes | 每条固定candidate都能安全处理的下游请求值 | Models v1与preflight可见 |
| Candidate forwarded includes | 当前Upstream API或Bridge实际接收/消费的值 | 私有execution事实，禁止序列化 |

当前唯一`ForwardOrOmit`值是`reasoning.encrypted_content`。其他include默认仍要求exact forward或explicit Bridge consumption；不得按Provider名称、任意字符串或`include`整个字段做宽泛忽略。

## 先失败的测试

实施前按顺序建立deterministic RED：

1. **Native omit RED**：Bailian/DeepSeek仅请求该hint时旧preflight失败；目标为接受且录制的upstream body不含该值。
2. **Candidate隔离RED**：synthetic固定Route中A原生支持、B仅安全省略；A在precommit失败后B收到独立过滤body，candidate顺序反转结果仍正确。
3. **Mixed include RED**：只删除approved hint，保留原生支持值和原顺序；包含任一未获批准值时整体拒绝并zero egress。
4. **Native forward回归**：ChatGPT/OpenAI等支持candidate精确保留该值。
5. **Bridge回归**：Responses→Chat继续显式消费hint，且不合成response item。
6. **Models/privacy回归**：公共`response_includes`包含hint，但Models不含forwarded set、Route、Provider或omission policy。

第1–2项必须在旧实现上按预期失败；不能先写helper再测试helper来冒充RED。

## 实施顺序

1. **类型策略**：在generation capability内建立逐值`Forward`/`ForwardOrOmit`闭合策略；不得加入用户可配置字符串。
2. **Compiler分层**：分别编译公共accepted set与每个`RouteExecutionCandidate`的私有forwarded set；启动校验拒绝越过Provider ceiling或无法消费的声明。
3. **Preflight保持**：preflight只读accepted set；未知include和不安全组合继续在任何egress前拒绝。
4. **Candidate过滤**：在`plan_request`的candidate body构造边界按forwarded set逐值过滤；删除空数组顶层字段，每个body独立immutable。
5. **特例收口**：删除已被统一策略替代的Bridge/Provider include特例，不保留双路径、legacy alias或Provider-name branch。
6. **Projection与隐私**：Models v1继续从accepted set投影；不新增schema字段、不泄漏private execution contract。
7. **证据与状态**：更新requirements、implementation status和tests；focused checks后运行完整Rust baseline并清空current focus。

## 明确非目标

- 不把`ForwardOrOmit`扩展到`web_search_call.action.sources`、file search、image URL、logprobs或未来include值。
- 不吞掉`parallel_tool_calls`、`prompt_cache_key`、tools、reasoning level、structured output或output limit。
- 不实施opaque encrypted-content replay、issuer affinity、跨credential/Target/Provider continuation。
- 不承诺Provider一定返回encrypted content，也不把plaintext reasoning解释为opaque continuation。
- 不做阶段2的Generation typed param/reason/validation-order重构。
- 不修改retry/fallback预算、timeout、precommit、EOF、SSE terminal或Images路径。
- 不升级Models v2，不添加compatibility shim、feature flag或runtime capability DSL。

## 验证顺序

先运行：

```text
cargo test --locked --test config_contract
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
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

另检查Models/OpenAPI/fixtures和Markdown links。只有修改`testdata/`或`tools/corpus/`时才追加Python/corpus baseline。真实Provider、Hermes runtime、负载与长期运行不属于首轮完成门；若执行必须单独记录有限证据。

## 完成判定与回滚

完成必须同时满足：Native omit RED与candidate隔离RED转绿；Native exact-forward与Bridge回归保持；mixed include继续fail closed；Models只暴露accepted contract；所有本地拒绝zero egress；requirements/status与live source一致；current focus恢复为空。

回滚单位是accepted/forwarded类型、compiler、planning过滤、测试和文档的完整阶段commit。不得只恢复preflight拒绝、留下虚假Models声明，也不得只回滚过滤而保留宽松accepted set。
