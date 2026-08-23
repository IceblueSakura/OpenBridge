# 阶段 6：Images profile algebra 与 registry conformance

> **状态：候选实施计划，不构成实施授权。** 只有阶段 5 的 response lifecycle 与 telemetry 证明完成后才能提升。本阶段以确定性 law/contract tests 证明当前 capability architecture；只有 RED 揭示真实缺陷时才修改生产代码。

## 1. 可观察结果

Images Public interface 始终是全部固定 candidates 的保守、可达交集：Target 不能提升 Provider ceiling，candidate顺序不改变公共能力，defaults/limits/sets组合不产生幽灵能力，Models projection不增加private contract没有的值，`(operation, task)`错误绑定启动失败。

## 2. 已验证基线与 owner

- `src/core/capability/images.rs::ImagesGenerationsCapabilities`与DashScope扩展拥有闭合validate/subset逻辑；`ImagesSizeDomain`另外拥有size-domain intersection。完整Images profile没有单独的`intersection`方法。
- `src/registry/public_model/compiler/aggregate.rs::aggregate_images_interface`才是多Route profile aggregation owner：它校验defaults一致，取`max_outputs`最小值，聚合size-domain intersection、response-format/parameter集合交集，并只在全部candidate一致时公开DashScope扩展。
- `src/registry/compiler.rs`校验Upstream API operation/task与Provider ceiling。
- `src/registry/public_model/compiler/`贡献、聚合并编译固定candidate公共interface。
- `src/registry/public_model.rs::ImagesInterfaceCapabilities`拥有Models/preflight同源访问器。
- 当前单candidate正常路径可用，但现有tests尚未系统证明代数规律、多candidate order与public projection。

## 3. 先失败的证明

先增加table/law tests；如果现有实现正确，它们可以直接为GREEN，但必须先对每条law记录当前未证明边界。至少覆盖：

1. `validate(x)`对所有注册可执行profile成功；非法default/domain/set拒绝。
2. `x ⊆ Provider ceiling`；兄弟Target不能通过另一个Target提升。
3. size domain的`intersection`满足幂等、交换、结合与subset；完整profile通过`aggregate_images_interface`验证重复输入幂等和candidate permutation invariance，不虚构不存在的profile-level method。
4. candidate顺序排列不改变公共interface或Models JSON。
5. 任一candidate缺少capability时，公共值关闭；default、set和limit分别求交后完整组合仍可达。
6. `(ImagesGenerations, ImageGeneration)`以外绑定启动失败。
7. public projection不暴露Provider/Target/Route、private extension或不存在的请求值。

若某条law RED，只修复最接近的algebra/compiler owner，并补最小回归；不得为了让测试通过收缩显然正确的Provider事实。

## 4. 实施步骤

1. 在pure capability tests建立最小、闭合、deny-all默认builder；避免宽松fixture掩盖必填事实。
2. 用确定性table覆盖profile与DashScope扩展的validate/subset，以及size domain的intersection laws。
3. 在registry contract通过`aggregate_images_interface`的真实Public Model编译入口构造多candidate，验证defaults/limits/sets聚合、order independence、Provider ceiling/Target narrowing和operation/task binding。
4. 比较compiled `ImagesInterfaceCapabilities`、Models v1 projection与preflight accessors，证明同源且无拓扑字段。
5. 为不可达default/limit组合增加启动失败；错误只含受信配置标识，不含credential或endpoint。
6. 只有现有抽象无法表达law时才做最小生产修复；不引入第二套profile或runtime capability DSL。
7. 更新implementation status的“已证明/未证明”边界；不要把未来Provider扩展写成当前支持。

## 5. 非目标

- 不引入`proptest`、新依赖或lockfile变更；组合规模触发前使用确定性table/law tests。
- 不升级Models v2，不引入Shared ModelIdentity、manifest或resource ledger。
- 不改Images transport/response lifecycle（阶段4–5 owner）。
- 不添加新Provider、格式、size或DashScope能力。

## 6. 验证

Focused：

```text
cargo test --locked --test config_contract
cargo test --locked --test images_forwarding_contract
cargo test --locked --test provider_contract
cargo test --locked --test provider_boundary_contract
```

另执行Models/OpenAPIfixture检查与完整Rust baseline。只有testdata/tools/corpus改变时才运行Python/corpus baseline。

## 7. 退出与回滚

完成门：所有law和registry conformance tests通过；任何production修复有对应RED；Models/preflight同源；无schema升级或依赖扩张；status更新且current focus清空。回滚以tests与最小修复的完整阶段commit为单位。
