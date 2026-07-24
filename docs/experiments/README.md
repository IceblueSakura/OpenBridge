# OpenBridge 原型实验记录

本目录记录用于验证或推翻设计假设的可重复实验。原型代码、单次手工测试或外部项目观察，只有在这里或等价文档中写明证据边界后，才能用于设计决策。

## 文件命名

```text
EXP-0001-short-description.md
```

编号一经分配不复用。实验结果失败、假设被推翻或后续失效时保留原文，并在状态和后续实验中说明。

## 状态

```text
Planned | Running | Confirmed | Refuted | Inconclusive | Superseded
```

`Confirmed` 只表示结果支持该实验的有限假设，不表示相关架构已经整体接受。

## 模板

```markdown
# EXP-0001：标题

## 状态

Planned

## 研究问题与假设

- Research question：
- Hypothesis：
- Affected decision：

## 环境

- OpenBridge commit：
- Client/version/commit：
- Provider/model/API version：
- OS/runtime：
- Configuration snapshot：

## Fixture 与步骤

1. `<step>`

Artifacts：

- 原始或脱敏 request：
- 原始或脱敏 response/SSE bytes：
- 客户端观察事件：
- 测试/脚本：

## 预期结果


## 观察结果


## 这证明什么


## 这不证明什么


## 结论

- Result：Confirmed / Refuted / Inconclusive
- Decision impact：
- Required follow-up：
- Revalidation trigger：
```

## 最低要求

- 固定 OpenBridge、目标客户端、Provider、模型和 API 版本；
- 保存原始或脱敏 wire evidence，而不只保存截图或总结；
- 成功路径同时覆盖对应的取消、错误或边界样本；
- fixture 不包含 credential、cookie、authorization code、prompt 中的私人数据或可重放 secret；
- 明确区分“目标客户端能消费模拟输出”和“真实 Provider 产生了该输出”；
- 实验影响设计时，在相关文档中链接实验并更新声明状态。

## 优先实验

- [EXP-0001：OpenAI SDK 原生 Chat/Responses tool-loop loopback](EXP-0001-openai-sdk-native-protocol.md)：已确认 SDK-first 的有限 Native Path 假设；不替代真实 Codex/Hermes/Provider corpus。

1. Codex custom Provider 的 Responses HTTP/SSE text/function-tool/cancel/error corpus，并记录 `supports_websockets = false` 的诊断证据；
2. Hermes Chat native text/function-tool/strict-endpoint corpus；
3. Native Path 与完整 IR round-trip 的未知字段保留比较；
4. Responses → Chat 并行 tool call、arguments delta 和 terminal assembly；
5. Chat → Responses usage-only final、item/response terminal 和 tool result replay；
6. Anthropic Messages content block、tool use/result 与 stop reason conformance；
7. continuation/state affinity 与首输出前 fallback 的负面实验。
