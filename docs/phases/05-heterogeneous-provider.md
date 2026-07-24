# C5 异构 Provider 验证

## 阶段目标

使用 Anthropic Messages 或同等级非 OpenAI wire dialect，反证 Provider Family、adapter、Bridge IR 和状态模型。

## 当前状态

`Not started`。

## 验证范围

- ordered content blocks；
- tool use/tool result identity；
- stop reason 和 response status；
- Provider-specific streaming events；
- error/retry 分类；
- native/bridge boundary；
- capability preflight；
- state 和 fallback。

## 测试条目

| ID | 测试 |
|---|---|
| C5-01 | text 与多 content blocks 的顺序和类型 |
| C5-02 | tool use/result 身份映射 |
| C5-03 | stream、stop、error、EOF 和 cancel |
| C5-04 | unsupported content block 在 egress 前拒绝 |
| C5-05 | router/pipeline 无 Provider 名称条件分支 |
| C5-06 | typed extension 有界且可审查 |
| C5-07 | native route 仍绕过 Bridge IR |
| C5-08 | conformance 与真实/脱敏 E2E corpus |

## 退出条件

- 异构 adapter 不要求核心 router 加 Provider-specific branch；
- 关键 content/tool/stop 语义可保留或明确拒绝；
- Bridge IR 不退化为所有 wire DTO 的并集；
- 若抽象被反证，先完成重构再增加 Provider。

## 关联模块

- [M03 Provider Adapter](../modules/03-provider-adapters.md)
- [M05 Protocol Bridge](../modules/05-protocol-bridge.md)
