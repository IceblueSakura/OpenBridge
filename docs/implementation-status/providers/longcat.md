# LongCat Provider 状态

## 当前实现

- Provider family 为 `longcat`，固定 origin 为 `https://api.longcat.chat`，使用 `longcat-primary` API-key pool。
- `LongCat-2.0` 提供 Chat/Responses Native 和两个显式 Bridge surface；Public Model 使用 `NativeFirst`。
- canonical reasoning 为 `none/high`，输出为 `PlainText`。Chat 将标准 effort 映射到
  `thinking.type=disabled/enabled`，Responses 保留标准 `reasoning.effort`。
- adapter 固定 LongCat 的 Models 与 generation 相对路径、API-key 认证和 Responses terminal discriminator。

## 所有权与确定性证据

- 注册与 wire 规则：[`src/providers/longcat/`](../../../src/providers/longcat/)。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs` 保护 endpoint、模型、认证和 terminal profile。
- `tests/forwarding_contract.rs` 与 Bridge tests 保护 Native/Bridge、JSON/SSE、tool continuation 和请求边界。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)覆盖
`LongCat-2.0` 的 Chat/Responses × JSON/SSE × `none/high`，当次单元均得到完整成功终态；`none` 无可观察 reasoning，
`high` 有 reasoning 证据。另有非流式 function call/result/final text 续接的定向成功记录。

## 未证明边界

其他 reasoning 档位、更多工具形状、外部 SDK/Agent、强制 Bridge/fallback、负载和长期运行未证明。官方参数说明和一次账号
请求不构成未来 Provider SLA。

## 相关文档

- [LongCat API 参考](../../references/providers/longcat/api.md)
- [Protocol Bridge](../features/protocol-bridge.md)
