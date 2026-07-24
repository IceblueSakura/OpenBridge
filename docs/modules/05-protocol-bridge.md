# M05 Protocol Bridge

## 目标

只在上下游 wire protocol 不一致时执行：

```text
source wire → source adapter → Bridge IR → target adapter → target wire
```

首个范围只覆盖文本、普通 function tool schema/call/result、usage 和必要终态。

## 核心规则

- 转换结果分为 `exact`、`structure_preserving`、`approximate`、`unsupported`；
- 不支持项在上游调用前拒绝；
- `call_id`、item id、response id、tool index 和 output index 不混用；
- 并行 tool calls 和 arguments delta 保持身份与顺序；
- 每个 stream 只有一个状态所有者；
- Bridge 不递归进入另一个 Bridge；
- hosted tool、opaque continuation/resource 和未知 item 默认不在首版支持。

## 实施方向

1. Responses → Chat；
2. Chat → Responses；
3. 用非 OpenAI dialect 反证 Bridge IR；
4. 对不能共同表达的语义使用有界 typed extension 或明确拒绝。

## 当前状态

已有详细设计和外部项目反例，运行时代码尚未实现。

首个 bridge 明确采用无状态 Agent Loop 兼容边界：只转换普通 function tool 的 call/result；需要 `previous_response_id`、opaque reasoning 或跨轮 tool-call 恢复的请求在上游调用前拒绝。后续 ledger 必须独立通过 issuer/deployment/TTL binding 门，不能采用全局 `call_id` 猜测。详见[Agent Loop 兼容与 Bridge 状态契约](../design/agent-loop-bridge-contract.md)。

## 详细资料

- [Chat/Responses Bridge 设计](../design/chat-responses-conversion.md)
- [Agent Loop 兼容与 Bridge 状态契约](../design/agent-loop-bridge-contract.md)
- [LiteLLM 协议分析](../research/litellm/chat-responses-analysis.md)
- [cc-switch 转换分析](../research/cc-switch/chat-responses-tool-conversion-analysis.md)
