# Generation 与 operation 剩余实现计划顺序

> **状态：候选计划族顺序，不构成除当前焦点之外的实施授权。** 本文只拥有待执行切片的顺序、依赖和提升门；详细行为仍由链接的设计与功能需求拥有。任何时刻只有
> [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md) 中的一项可实施。

## 1. 排序原则

1. 先修复已由锁定依赖和 live source 证明会截断合法 Responses SSE 的 timeout 缺陷。
2. 再解除 `reasoning.encrypted_content` 对 Hermes/Bailian/DeepSeek 的确定性本地阻断，不放宽其他 capability。
3. 在 include 首错消失后统一 Generation 字段级错误，避免把过时首错固化进公共合同。
4. 将改变 stream commit、EOF 和 retry/fallback 的高风险语义留在独立焦点。
5. 等共享 timeout 与 streaming 边界稳定后，再分片关闭 Images 执行证明、profile algebra 和 legacy 债务。

计划之间不得并行实施，也不得把后续计划的便利性需求带入当前切片。改变本顺序需要重新核对 live source、风险和用户优先级，并显式更新本文；不能由实现者自行扩大 `current-focus.md`。

## 2. 有序计划族

| 计划 | 可观察结果 | 详细设计 owner | 前置条件 | 完成门 |
|---|---|---|---|---|
| **1. Responses streaming timeout policy 与归因** | 持续产生合法 event 并最终到达 terminal 的 stream 不再被普通非流式 total deadline 截断；headers、首 event、event idle、stream total 和非流式 total 分开表达 | [timeout 设计：切片 A](responses-stream-premature-termination-and-timeouts.md#切片-a修正-total-deadline-与增加归因) | 无；执行前重新核对锁定 reqwest、Target 注册和工作树 | focused transport/SSE/replay/observability tests 与 Rust baseline 通过；更新实施现状并清空焦点 |
| **2. `reasoning.encrypted_content` 精确兼容提示** | 所有 Responses Public Model 安全接受该精确 hint；candidate 原生支持时保留，不支持时只删除该值；其他 include 继续 fail closed | [`reasoning.encrypted_content` 设计](responses-reasoning-encrypted-content-compatibility.md) | 计划 1 完成并清空焦点；没有代码级依赖 | accepted/forwarded 集合分离、candidate body 独立、Bridge 收口、zero-egress 与不伪造输出证据完整 |
| **3. Generation capability 字段级错误** | Generation 本地拒绝保留稳定 status/type/code，并返回一个确定性的标准 `param`；首错顺序不受 candidate 或集合遍历影响 | [错误定位设计](generation-capability-error-diagnostics.md) | 计划 2 完成，使 DeepSeek 剩余首错稳定；先把 `param` 与首错顺序写入正式需求 | typed param/reason、固定 validation order、OpenAPI/需求/测试一致，所有拒绝保持 zero egress |
| **4. Responses stream precommit 与 EOF 可见失败** | 首个 event 前的失败可返回 HTTP error 并按既有 policy retry/fallback；commit 后 transport error 与 terminal 前 EOF 对客户端表现为 body failure，不伪造 terminal | [timeout 设计：切片 B](responses-stream-premature-termination-and-timeouts.md#切片-bprecommit-与-eof-可见失败) | 计划 1 的 timeout taxonomy 和观察边界稳定；计划 2–3 已关闭直接 Hermes 阻断 | canonical replay/corpus、SSE、retry/fallback、取消和外部 loopback 全部满足新语义 |
| **5A. Images timeout 与单 attempt 生命周期** | Images timeout 稳定映射为 504 `upstream_timeout`；单次不可重放 attempt 的 coordinator 归属、取消和 accounting 有明确合同 | [Images 剩余证明](capability-operation-refactor/07-remaining-proof-and-cleanup.md#images-剩余执行证明) | 计划 1 的共享 timeout 分类稳定 | operation-specific timeout、取消、attempt/commit tests 通过；不引入自动 retry/fallback |
| **5B. Images response 与 telemetry 证明** | response body 超限、读取失败、提前 EOF、下游取消及 commit 后行为有专项证据；prompt、URL 和上游 body 不进入普通 telemetry | [测试与证据准备](capability-operation-refactor/08-testing-evidence-and-readiness.md) | 5A 完成 | lifecycle 与 observability focused tests 通过，状态页只记录实际证据 |
| **5C. Images profile algebra 与 registry conformance** | subset、intersection、candidate order、public projection、Provider ceiling/Target narrowing 和错误绑定受确定性 law tests 保护 | [测试与证据准备](capability-operation-refactor/08-testing-evidence-and-readiness.md#2-profile-algebra-必测性质) | 5B 完成；仍使用 table/law tests | profile algebra、registry 与 Models projection tests 通过；不无依据引入 property-testing 依赖 |
| **5D. Operation legacy 收口** | 对旧 capability/module/type、alias、重复 builder、orphan fixture 和 stale link 完成可复核删除审查 | [legacy 清理](capability-operation-refactor/07-remaining-proof-and-cleanup.md#legacy-清理) | 5A–5C 全部完成 | 清单逐项归属；只删除已证明残留；full baseline、OpenAPI/link 和 `git diff --check` 通过 |

`Models v2`、Shared `ModelIdentity`、resource ledger、build-time manifest 和 property testing 继续服从
[后续决策门](capability-operation-refactor/09-open-questions.md)，当前不属于待提升计划。

## 3. 跨计划所有权

- 计划 1 只拥有 generation streaming timeout policy、body timeout 安全分类和最小归因；不改变 precommit/EOF 客户端语义。
- 计划 4 只拥有 precommit、terminal 前 EOF、commit 后 body failure 与其 retry/fallback 结果；不重新设计 timeout 配置。
- 计划 5A 复用计划 1 的 timeout taxonomy，但 Images 504、不可重放和单 attempt accounting 仍由 Images operation 自己证明。
- 计划 2 拥有 `reasoning.encrypted_content` 的 accepted/forwarded policy；计划 3 拥有所有 Generation `param`、内部 reason 和固定首错顺序。
- 计划 5D 只能删除前序证明已经替代的路径；名称含 `legacy`、`alias` 或旧协议并不自动构成删除依据。

## 4. 每项计划的提升流程

1. 重新读取 live source、工作树、对应功能需求和实施状态。
2. 只把表中的一个可观察结果写入 `current-focus.md`，同时写明 requirement、RED、非目标和验证边界。
3. 先建立在旧代码上按预期失败的 deterministic test 或 fixture，再做 direct replacement；禁止长期双路径。
4. 先运行 focused validation，再运行与改动相称的 Rust baseline；只有触及 corpus 时才追加 Python/corpus baseline。
5. 将确认事实和实际命令写入最接近的 implementation-status owner，删除或降级已完成的执行前条目，然后恢复空焦点。
6. 下一计划必须重新建立基线；前一计划完成不自动授权后一计划。
