# 阶段 7：Operation legacy 删除审查与收口

> **状态：候选实施计划，不构成实施授权。** 只有阶段 4–6 全部完成后才能执行。名称含 `legacy`、`alias` 或旧协议不自动证明可删除。

## 1. 可观察结果

对 capability/operation 重构后的残留完成一份可复核、证据驱动的全仓库审查：删除已被唯一现行路径完全替代的旧module/type/alias/builder/fixture/link；合法双时代协议、MCP兼容边界和仍被调用的转换保留并记录owner。没有可删除残留时，阶段可以以“检索完整、零删除”结束，不制造清理。

## 2. 前置条件与审查范围

阶段 4–6 已证明Images attempt、response lifecycle、telemetry、profile algebra和registry conformance；否则无法判断旧路径是否真正被替代。

检索范围：

- old capability/module/type names；
- compatibility conversions与unused aliases；
- operation-only API key或固定private operation fields；
- registration media mutation；
- duplicate test builders、orphan fixtures、stale links；
- 注释、OpenAPI、requirements/status中指向已删除owner的引用。

`target/`、private config、generated corpus output和凭据文件不进入审查。

## 3. 证据清单

每个候选项必须记录：symbol/path、定义、全部调用点、现行replacement、公共/私有可见性、删除风险、验证命令与结论。分类仅允许：

1. **删除**：无合法调用，且前序阶段已证明replacement完整；
2. **保留**：属于当前协议/兼容/测试owner；
3. **阻塞**：证据不足，留待独立current focus，不在本阶段猜测。

先用source/search/compiler证据建立清单，再写删除RED或编译/fixture断言。禁止仅凭名称或LOC做决定。

## 4. 实施步骤

1. 从`docs/implementation-status/current-architecture.md`和live module graph建立owner map。
2. 搜索候选定义与所有usage，区分production、test-only、public re-export、wire compatibility和dead path。
3. 对每个“删除”项先建立replacement coverage或确认现有tests已覆盖其行为，然后执行最小删除。
4. 同步移除orphan import/re-export、duplicate builder、fixture和stale doc link；不顺带重命名合法API。
5. 运行编译/测试后再次全仓库扫描候选字符串，确认没有半迁移或dead branch。
6. 更新current architecture与最接近的status，只记录当前owner和实际删除结果；不保存长篇决策历史。
7. 若发现需要公共schema、Provider contract或behavior变化，停止并把它拆成新的current focus，不在清理阶段实施。

## 5. 非目标

- 不做无证据重构、格式化、性能优化或模块重排。
- 不删除MCP双时代协议、Provider兼容层或历史命名，仅因它们看似“旧”。
- 不引入compatibility shim、deprecated alias、feature flag或TODO代替删除。
- 不升级Models/OpenAPI schema，不修改credential/config。

## 6. 验证

至少执行：

```text
cargo fmt -- --check
cargo check --locked --all-targets
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

另检查OpenAPI、全部tracked Markdown文件的相对links/anchors、canonical fixtures和全仓库残留字符串；涉及corpus才追加Python/corpus baseline。

## 7. 退出与回滚

完成门：清单逐项有结论；所有删除有replacement证据；阻塞项未被偷偷实现；全量baseline、OpenAPI/link检查通过；architecture/status一致；current focus清空。回滚按该阶段单一commit恢复删除与文档，不保留半删除状态。
