# 09：开放问题与决策门

本页只记录执行前尚需用户确认或在 live source 上重新验证的决策。推荐默认不是实施授权；到达指定阶段前必须关闭对应问题。

## 1. Models v2 interface 容器

- **问题**：`interfaces` 使用 operation-keyed object，还是 tagged array？
- **推荐**：operation-keyed object；读取直接、顺序由 stable operation name 决定，现有客户端心智接近。
- **决策截止**：阶段 3 前。

## 2. Canonical task set 表达

- **问题**：typed struct of options，还是 unique vector of closed variants？
- **推荐**：non-empty unique vector；新增 task 不需要扩大一个 god struct，启动校验负责唯一性。
- **约束**：不允许 string map；每个 variant 独占 payload。
- **决策截止**：阶段 1 前。

## 3. Upstream API task binding

- **问题**：operation 是否足以推导 task？
- **推荐**：显式 `task_binding`。当前 Chat 可承载普通 generation、ASR、TTS、VoiceDesign、VoiceClone，不能从 operation 猜测。
- **决策截止**：阶段 1 前。

## 4. Route transform

- **问题**：保留 `Native/Bridged`，还是显式 bridge kind？
- **推荐**：`Native | GenerationBridge(direction)`；未来转换必须新增闭合 variant，不能复用笼统 `Bridged`。
- **决策截止**：阶段 1 前。

## 5. Media limit newtypes

- **问题**：继续使用命名 `u32` 字段，还是引入 `MaxUrlBytes/EncodedBytes/DecodedBytes/MaxParts` newtypes？
- **推荐**：对容易互换的 byte/part 单位使用 checked newtype；对简单 count 可保留明确字段。
- **决策截止**：阶段 2 前。

## 6. Resource-backed image/file ID

- **问题**：何时开放 `file_id`？
- **推荐**：在 issuer、credential owner、Target/API affinity、lifecycle 和 fallback 规则完整前保持 unsupported；不在本重构中顺带实现 ledger。
- **决策截止**：首个 resource capability current focus 前。

## 7. Attempt engine 抽象程度

- **问题**：万能 async operation trait，还是共享 coordinator + operation-owned driver？
- **推荐**：共享 `AttemptCoordinator`，operation driver 保留 request/response/framing/retry eligibility；不使用 runtime plugin 或万能 AST。
- **决策截止**：阶段 4 前。

## 8. 首个纵向证明切片

- **问题**：typed file input、标准 Images endpoint、标准 Audio endpoint，哪一个先实现？
- **推荐默认**：typed file input 的 inline + remote Native 子集，因为它直接证明 file profile、media envelope、Models v2 和 zero-egress resource boundary。
- **替换条件**：用户有更高优先级的具体 endpoint/provider，并能给出 wire、限制和验收证据。
- **决策截止**：阶段 5 前。

## 9. Build-time manifest/code generation

- **问题**：Provider/model profile 增长后是否改用 checked manifest？
- **推荐**：当前继续 Provider-local Rust constants；只有大量文件已退化为纯数据且 schema 稳定时，才评估 checked-in provider-local manifest → build-time typed Rust generation。
- **约束**：不引入运行时 capability DSL 或动态注册。
- **决策截止**：出现明确维护痛点时，不作为当前阶段前置。

## 10. Property testing 依赖

- **问题**：profile algebra 是否立即引入 `proptest`？
- **推荐**：先用确定性 table/law tests；组合规模增长或发现交集边界遗漏后，再有意更新 `Cargo.toml/Cargo.lock`。
- **决策截止**：阶段 2 测试设计时。

## 11. 需要重新核验的事实

开始执行前重新确认：

- live `OperationKind`、task variants、Models schema version 和 Provider registration 数量；
- 当前 `cargo fmt/test/clippy` 基线；
- OpenAI 官方当前 Images/Audio/Files wire（只在相应 operation 进入 current focus 时）；
- 目标 Provider 的 source/format/limit/state/resource 事实；
- corpus schema 与 testkit 当前版本。

设计包中的日期、模型和测试数量不能替代执行时重新核验。
