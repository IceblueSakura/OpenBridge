# Generation 与 operation 剩余七阶段实现计划

> **状态：候选计划族顺序，不构成除当前焦点之外的实施授权。** 本文只拥有待执行切片的顺序、依赖和提升门；详细行为仍由链接的设计与功能需求拥有。任何时刻只有
> [`implementation-plans/current-focus.md`](../implementation-plans/current-focus.md) 中的一项可实施。

## 1. 排序原则

1. 先解除 `reasoning.encrypted_content` 对 Hermes/Bailian/DeepSeek 的确定性本地阻断，不放宽其他 capability。
2. 在 include 首错消失后统一 Generation 字段级错误，避免把过时首错固化进公共合同。
3. 将改变 stream commit、EOF 和 retry/fallback 的高风险语义留在独立焦点。
4. 复用当前 shared timeout taxonomy，再分片关闭 Images 执行证明、profile algebra 和 legacy 债务。

计划之间不得并行实施，也不得把后续计划的便利性需求带入当前切片。改变本顺序需要重新核对 live source、风险和用户优先级，并显式更新本文；不能由实现者自行扩大 `current-focus.md`。

## 2. 有序计划族

| 阶段 | 可观察结果 | 实施计划 | 详细设计 owner | 前置条件 | 完成门 |
|---|---|---|---|---|---|
| **1** | 所有 Responses Public Model 安全接受 `reasoning.encrypted_content`；candidate 原生支持时保留，不支持时只删除该值；其他 include 继续 fail closed | [阶段 1](implementation-plans/01-reasoning-encrypted-content-compatibility.md) | [兼容提示设计](responses-reasoning-encrypted-content-compatibility.md) | 无；执行前重新核对 Models/preflight/planning 与 Provider ceiling | accepted/forwarded 分离、candidate body 隔离、planning-owned Bridge omission、zero-egress 与不伪造输出 |
| **2** | Generation 本地拒绝保留稳定 status/type/code，并返回确定性的标准 `param`；首错顺序不受 candidate 或集合遍历影响 | [阶段 2](implementation-plans/02-generation-capability-error-diagnostics.md) | [错误定位设计](generation-capability-error-diagnostics.md) | 阶段 1 完成；先把 `param` 与首错顺序写入正式需求 | typed param/reason、固定 validation order、OpenAPI/需求/测试一致，拒绝保持 zero egress |
| **3** | 首 event 前失败可返回 HTTP error并按既有policy处理；commit后transport error与terminal前EOF表现为body failure | [阶段 3](implementation-plans/03-responses-stream-precommit-and-eof.md) | [同计划的状态机与 lifecycle 合同](implementation-plans/03-responses-stream-precommit-and-eof.md#4-候选状态机与实施步骤) | 阶段 1–2 完成；当前 timeout taxonomy 稳定 | replay/corpus、SSE、retry/fallback、取消和外部 loopback满足新语义 |
| **4** | Images timeout 稳定映射为 504；单次不可重放 attempt 的 coordinator、取消和 accounting 有明确合同 | [阶段 4](implementation-plans/04-images-timeout-and-attempt-lifecycle.md) | [Images 剩余证明](capability-operation-refactor/images-proof-and-legacy-cleanup.md#images-剩余执行证明) | 阶段 3 完成；复用共享 timeout 分类 | operation-specific timeout、取消、attempt tests通过；不引入retry/fallback |
| **5** | response body 超限、读取失败、提前 EOF、取消及 commit 行为有专项证据；敏感内容不进入普通 telemetry | [阶段 5](implementation-plans/05-images-response-and-telemetry.md) | [测试与证据准备](capability-operation-refactor/testing-evidence-and-readiness.md) | 阶段 4 完成 | lifecycle/observability tests通过，status只记录实际证据 |
| **6** | subset、intersection、candidate order、public projection、ceiling/narrowing与错误绑定受确定性 law tests保护 | [阶段 6](implementation-plans/06-images-profile-algebra-and-registry-conformance.md) | [profile algebra](capability-operation-refactor/testing-evidence-and-readiness.md#2-profile-algebra-必测性质) | 阶段 5 完成；使用table/law tests | profile algebra、registry与Models projection tests通过；不引入property-testing依赖 |
| **7** | 对旧 capability/module/type、alias、重复 builder、orphan fixture和stale link完成可复核删除审查 | [阶段 7](implementation-plans/07-operation-legacy-cleanup.md) | [legacy 清理](capability-operation-refactor/images-proof-and-legacy-cleanup.md#legacy-清理) | 阶段 4–6 全部完成 | 清单逐项归属；只删除已证明残留；full baseline、OpenAPI/link与diff check通过 |

`Models v2`、Shared `ModelIdentity`、resource ledger、build-time manifest 和 property testing 继续服从
[后续决策门](capability-operation-refactor/future-decision-gates.md)，当前不属于待提升计划。

## 3. 跨计划所有权

- 阶段 3 只拥有 precommit、terminal 前 EOF、commit 后 body failure 与其 retry/fallback 结果；不重新设计当前 timeout 配置。
- 阶段 4 复用当前 timeout taxonomy，但 Images 504、不可重放和单 attempt accounting 仍由 Images operation 自己证明。
- 阶段 1 拥有 `reasoning.encrypted_content` 的 accepted/forwarded policy、candidate filtering 与 Bridge 前 omission；阶段 2 拥有所有 Generation `param`、内部 reason 和固定首错顺序。
- 阶段 7 只能删除前序证明已经替代的路径；名称含 `legacy`、`alias` 或旧协议并不自动构成删除依据。

## 4. 每项计划的提升流程

1. 重新读取 live source、工作树、对应功能需求和实施状态。
2. 只把表中的一个可观察结果写入 `current-focus.md`，同时写明 requirement、RED、非目标和验证边界。
3. 先建立在旧代码上按预期失败的 deterministic test 或 fixture，再做 direct replacement；禁止长期双路径。
4. 先运行 focused validation，再运行与改动相称的 Rust baseline；只有触及 corpus 时才追加 Python/corpus baseline。
5. 将确认事实和实际命令写入最接近的 implementation-status owner，删除或降级已完成的执行前条目，然后恢复空焦点。
6. 下一计划必须重新建立基线；前一计划完成不自动授权后一计划。
