# 08：测试、证据与执行准备

## 1. 测试所有权

| 层 | 负责证明 | 不负责证明 |
|---|---|---|
| Pure profile tests | validate、subset、intersection、reachability、projection | Router、Provider wire |
| Registry contract | reference、task/operation、ceiling/narrowing、fixed candidates | 真实 upstream |
| In-process forwarding | auth、analysis、preflight、planning、exact egress、zero egress、terminal | 真实 Provider 质量 |
| Replay/lifecycle | HTTP error、retry/fallback、commit、cancel、EOF/SSE | capability facts 正确性 |
| Python corpus/testkit | canonical wire、fragmentation、fixture/schema/provenance | Route/credential/retry 实现 |
| Loopback SDK/client | 真实下游 HTTP 与 SDK shape | 外部 Provider SLA |
| Real Provider evidence | 固定账号/模型/payload 的定向行为 | 其他账号、fallback、负载、长期稳定性 |

保持一个 canonical wire fixture；断言放在最接近 owner 的层，不在 Rust/Python/文档三处复制同一事实。

## 2. Profile algebra 必测性质

每个 operation/profile 至少保护：

- `validate(x)` 对所有可执行 profile 成功；
- `x ⊆ ceiling`，Target 不能提升 Provider；
- `intersection(x, x) == x`；
- `intersection(x, y) == intersection(y, x)`；
- candidate 顺序不改变能力交集；
- 任一 candidate 缺少能力时，公共能力按规则关闭；
- 集合、default、limits 分别求交后，完整组合仍可达；
- public projection 不增加 private contract 没有的值。

首轮可用确定性 table tests，不立即增加 property-testing 依赖；当 profile 组合数量明显增长时再评估 `proptest`。

## 3. Provider/Target conformance

每个新增 Provider/Target 至少需要：

1. Provider ceiling 与 Target subset compile test；
2. compiled Models projection；
3. 一个合法 request exact egress；
4. 一个关键 unsupported capability zero egress；
5. response terminal/error contract；
6. credential/header/origin boundary；
7. 若有 fallback，验证固定顺序、首输出 commit 和取消。

Provider profile 常量必须 deny-by-default；新增 ceiling 不能提升未显式选择的兄弟 Target。

## 4. Operation 纵向合同

每个新增 operation 必须覆盖：

- endpoint/method/auth/content type；
- strict request field catalog；
- body、part、inline/remote 和 response budget；
- Native request/response fidelity；
- JSON/SSE/binary terminal；
- 4xx/5xx/损坏 body/EOF；
- retry eligibility、replay budget、commit、cancel；
- operation label、usage units 和敏感数据排除；
- Models interface 与 preflight 同源；
- Bridge 或 resource affinity 的明确允许/拒绝。

## 5. Test 目录建议

保留少量 integration crate，内部按 domain/provider 分模块，避免每个小文件重复编译整个 crate：

```text
tests/
  registry_contract.rs
  registry_contract/{operation,task,media,public_model}.rs
  forwarding_contract.rs
  forwarding_contract/{generation,embeddings,media,providers}.rs
  provider_contract.rs
  provider_contract/<provider>.rs
  lifecycle_contract.rs
  support/builders/{model,provider,target,route}.rs
```

Builder default 必须最小、闭合、deny-all；不得用宽松默认隐藏必填 capability。

## 6. 每阶段验证门

最小 Rust baseline：

```text
cargo fmt -- --check
cargo check --locked --all-targets
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

涉及 corpus：

```text
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

涉及 OpenAPI/Models schema：追加 standard/extended list/retrieve、OpenAPI delivery、example fixture 和 topology privacy tests。

## 7. 执行准备清单

进入任一阶段前必须确认：

- [ ] live checkout 与工作树已重新检查；
- [ ] 当前格式和测试基线全绿，或既有失败已记录；
- [ ] 只选择一个可观察切片进入 `current-focus.md`；
- [ ] 需求、失败语义、安全/资源边界和非目标已明确；
- [ ] RED 在旧代码上按预期失败；
- [ ] direct replacement 的删除清单已列出；
- [ ] OpenAPI/DTO/fixture 影响已识别；
- [ ] 不读取或修改私有 credential/config；
- [ ] 外部 SDK、真实 Provider、负载和长期验收边界已分层；
- [ ] 回滚方式是阶段 commit/revert，不依赖 runtime dual path。
