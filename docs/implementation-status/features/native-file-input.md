# 功能：Typed Native 文件输入

## 当前行为

- Chat Completions 与 Responses 使用彼此独立的 typed file profile。Chat profile 描述 nested `file` inline data；Responses profile 描述 `input_file` inline data、external HTTPS URL 与 PDF `detail` domain。
- Analyzer 只在协议定义的 user input 位置识别 file part，冻结 source、encoding、PDF media type、explicit detail、part count、filename/URL length 与 inline encoded/decoded byte facts；不保留 filename、URL 或 Base64 内容。
- Source one-of、canonical Base64/data URL、`.pdf`/`application/pdf` 一致性、absolute HTTPS 与 local/reserved IP literal、filename/URL/part/byte budgets 都在首次 egress 前验证。`file_id` 在没有 issuer/owner affinity 时 fail closed。
- Native planning 保持原始 part/item 顺序、filename、data/URL、detail 与 terminal。OpenBridge 不下载、解析、转换、缓存文件，也不签发 resource identity。
- Models v1 的 `multimodal_input.file` 投影与 private preflight 来自同一保守交集。Chat projection 的 `detail` 为 `null`；Responses 可投影 `auto|low|high` 的 default/allowed domain。
- Generation Bridge 对 file 始终贡献空能力。OpenAI family ceiling 描述标准 API wire 上限，但所有 checked-in executable Targets 仍显式选择 `file: None`，因此当前生产 Public Models 均不公开或接受 file input。

## 所有权

Profile algebra 位于 `src/core/capability/generation/media.rs`；OpenAI family ceiling 位于 `src/providers/openai/media.rs`；Models 投影与交集位于 `src/registry/public_model.rs`；request analysis/preflight 位于 `src/pipeline/generation/analysis/file_input.rs` 与 `src/pipeline/generation/preflight.rs`。

## 确定性证据

- `tests/forwarding_contract/file_input.rs`：synthetic Native Chat/Responses Models projection、raw Base64/data URL/HTTPS exact egress、source/media/detail/limit/file_id zero egress、生产 Models deny-by-default 与 Bridge fail closed。
- `src/core/capability/generation/media.rs` 单元测试：file profile source-specific limit 与 subset ordering。
- `cargo test --locked --test forwarding_contract`：通过（79 tests）。
- `cargo test --locked --test ingress_contract`：通过（6 tests，包含内置 OpenAPI 资源交付检查）。
- `cargo fmt --all -- --check`、`cargo check --locked --all-targets`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check`：全部通过。
- 本切片涉及的 Markdown relative links：本地检查通过。未修改 `testdata/` 或 `tools/corpus/`，未运行 Python corpus suite。

## 外部证据与未证明范围

当前只重新核对了 2026-08-21 的 OpenAI API reference 与 File inputs guide，未调用真实 OpenAI/ChatGPT Provider。Synthetic loopback 不证明任一真实模型、账号或 backend 支持 file，也不证明 Provider-side DNS、redirect、下载大小、MIME、解析质量、费用、SDK/Agent、负载或长期运行。

## 相关文档

- [Native 文件能力需求](../../functional-requirements/extended-capabilities/native-file.md)
- [OpenAI Chat 文件输入参考](../../references/openai/files/chat-input.md)
- [OpenAI Responses 文件输入参考](../../references/openai/files/responses-input.md)
- [Models 与能力预检](models-api-and-capability-preflight.md)
