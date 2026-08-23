# 阶段 2：Generation capability 字段级错误

> **状态：候选实施计划，不构成实施授权。** 只有阶段 1 完成并清空 current focus 后才能重新核验并提升。详细语义见
> [Generation capability 错误定位设计](../generation-capability-error-diagnostics.md)。

## 1. 可观察结果

Generation 本地拒绝继续使用现有稳定 HTTP status、`error.type` 和 `error.code`，同时返回一个确定性的标准顶层 `param`。相同请求、registry 与 interface 在 candidate 顺序、集合遍历和构建变化下必须选择同一首错；所有本地拒绝保持 zero egress。

## 2. 前置条件与 owner

- 只有阶段 1 实施完成并清空 current focus 后，`reasoning.encrypted_content` 才不再成为过时首错，DeepSeek/Hermes 请求的剩余能力错误才可作为稳定 RED。
- `src/pipeline/error.rs::RequestPlanningError` 当前混合无字段 capability variants、`UnknownParameter(String)` 与少量 `UnsupportedParameter`。
- `src/pipeline/generation/preflight.rs::validate_interface_request` 当前把多个无关 capability checks 合并成布尔表达式。
- `src/ingress/response.rs` 当前把多个 Generation variants 映射为无 `param` 的统一错误。
- Embeddings/Images typed error 可作为结构先例，但其合同不属于本阶段。

正式实施前，必须先把 Generation `param` 与确定性首错顺序写入功能需求；内部 reason 不默认成为公共 wire。

## 3. RED

1. 使用不含 `include` 的 synthetic request，仅请求 interface 不支持的 `parallel_tool_calls:true`；旧实现返回无 `param` 的 capability 400，目标为 `param=parallel_tool_calls`。
2. 阶段 1 完成后加入脱敏 Hermes DeepSeek shape：hint 被安全处理后，剩余首错稳定为 `parallel_tool_calls`；修复该字段后下一首错按正式顺序出现。
3. 为 tool choice、tools、structured output、reasoning、reasoning level/summary、streaming、output limit、multimodal input 和 continuation 建立表驱动拒绝，旧实现缺少具体字段。
4. 同一请求同时违反多个字段时，变换 JSON key 顺序、candidate 顺序和集合构造，目标 `param` 不变。
5. 每个 rejected case 断言 transport 未调用；OpenAI-compatible error envelope 保持。

## 4. 实施步骤

1. 在功能需求中固定公共字段定位和 validation order；确认顶层标准字段命名，不把内部 capability 类型暴露为 `param`。
2. 将 Generation planning error 收敛为 typed param + closed internal reason，直接替换无字段的 capability variants；避免兼容 alias。
3. 拆开 `validate_interface_request` 的合并布尔表达式，按固定顺序逐项返回首个错误。
4. 让 limit、reasoning、multimodal 与 state 分支携带其 owning request field；特别保留触发有效 output limit 的实际源字段，不能只保存合并后的最大值。对象内部失败仍定位到稳定顶层参数。
5. 更新 `src/ingress/response.rs`，保留既有 status/type/code/message 边界并序列化标准 `param`；内部 reason 只用于低基数测试/trace，默认不下发。
6. 原子更新 OpenAPI error examples、requirements、fixtures、focused tests 与 implementation status。
7. 扫描重复错误模型，只删除已由 typed path 替代的 Generation variant；不改 Embeddings/Images wire。

## 5. 非目标

- 不改变任何 Model/Provider capability truth、Route 顺序或 ignored-parameter policy。
- 不吞掉 `parallel_tool_calls`、prompt-cache 或其他字段。
- 不一次返回所有错误，也不暴露 candidate、Provider 或 Route。
- 不修改 Images/Embeddings 已有错误合同。
- 不实施阶段 3 的 streaming commit/EOF 变化。

## 6. 验证

Focused：

```text
cargo test --locked --test forwarding_contract
cargo test --locked --test bridge_forwarding_contract
cargo test --locked --test ingress_contract
cargo test --locked --test observability_contract
```

同时运行 OpenAPI/fixture 检查与完整 Rust 基线：`cargo fmt -- --check`、`cargo check --locked --all-targets`、`cargo test --locked`、`cargo clippy --locked -- -D warnings`、`git diff --check`。

## 7. 退出与回滚

完成门：每个主要 capability family 有字段级 RED；首错顺序稳定；zero egress；OpenAPI/需求/实现一致；阶段 1 行为不回退；current focus 清空。回滚必须覆盖 typed error、preflight order、ingress mapping、OpenAPI 和 tests 的完整阶段。
