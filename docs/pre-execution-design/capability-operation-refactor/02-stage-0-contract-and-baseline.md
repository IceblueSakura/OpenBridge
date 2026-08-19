# 02：阶段 0——合同与基线

## 目标

在改动生产类型前冻结长期不变量、当前行为基线和测试结构，使后续每次 direct replacement 都能区分回归、预期破坏和既有失败。

本阶段不新增 operation、不开放 file/video/resource 能力，也不建立空生产接口。

## 前置条件

- live checkout、功能需求、实施现状和工作树重新核对；
- 本设计包中的开放问题完成阶段 0 必需决策；
- 只把一个可观察准备切片写入 `implementation-plans/current-focus.md`。

## 工作项

1. 修复当前已知的 `cargo fmt -- --check` 基线阻塞，使格式检查成为硬门。
2. 为现有 Chat、Responses、Embeddings 建立 operation contract 基线：
   - request field classification；
   - Models interface projection；
   - preflight zero egress；
   - Native wire fidelity；
   - Bridge media/state reject；
   - response terminal、commit、retry、cancel；
   - operation telemetry allowlist。
3. 整理 synthetic Provider 和 loopback upstream builders；默认能力必须是 deny-all，所有能力显式开启。
4. 记录当前扩展 Models JSON 的 canonical fixture，作为未来 v2 破坏性变更的 RED 对照，而不是永久兼容承诺。
5. 为 capability profile 增加纯函数测试入口：validate、subset、intersection、public projection。

## 先失败测试

阶段 0 的测试应保护当前合同或暴露缺少的架构门，不应要求尚未实施的未来 endpoint 成功。优先新增：

- Provider ceiling 新增媒体时，未显式选择的 Target 不得自动提升；
- Bridge candidate 必须对媒体贡献空能力；
- Models 声称支持的 capability 必须能够通过相同 private contract 完成 preflight；
- mixed candidates 的交集必须重新通过 profile reachability validation。

若测试只能通过预先加入未来空变体、feature flag 或假 handler，应删除该测试并缩小范围。

## 删除清单

本阶段原则上不删除生产行为，但应删除：

- 测试 builder 中会隐式开启能力的宽松 default；
- 重复且无 owner 的 JSON fixture；
- 已失效的测试注释或与 live source 冲突的断言。

## 退出门

```text
cargo fmt -- --check
cargo check --locked --all-targets
cargo test --locked
cargo clippy --locked -- -D warnings
git diff --check
```

若修改 `testdata/` 或 `tools/corpus/`，追加：

```text
uv lock --check --project tools/corpus
uv run --project tools/corpus pytest tools/corpus/tests
uv run --project tools/corpus corpus --root testdata lint
```

阶段完成后，确认事实进入 implementation status，`current-focus.md` 恢复为空。

## 非目标

- 不预建 Images/Audio/Files/Video/Realtime operation；
- 不执行真实 Provider 或 SDK 验收；
- 不开始 Models v2；
- 不重构 pipeline 或 adapter。
