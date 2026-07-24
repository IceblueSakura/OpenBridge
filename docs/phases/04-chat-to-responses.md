# C4 Chat → Responses Bridge

## 阶段目标

让 Hermes Chat transport 通过 Responses-only Provider 完成最小文本和 function-tool loop。

## 当前状态

`Blocked by C3`。依赖 C3 的 Bridge IR、状态和错误边界先通过真实 E2E。

## 进入条件

- C3 已 `Accepted`；
- Bridge IR、identity、reject matrix 和 stream lifecycle 已由 Codex → Chat-only Provider E2E 验证；
- Hermes Chat 固定版本 corpus 与一个授权 Responses-only Provider 环境可重跑。

## 首版范围

- system/developer/user/assistant/tool message；
- function tools；
- assistant `tool_calls[]` 和 tool result；
- text/tool stream；
- usage、error、cancel 和 terminal；
- stateful continuation 默认拒绝。

具体的 Agent/tool 边界、identity、state owner、stream lifecycle 和 future ledger 门见[Agent Loop 兼容与 Bridge 状态契约](../design/agent-loop-bridge-contract.md)。

## 非目标

- 重写 C3 已接受的 Bridge IR，除非反向 corpus 提供明确反证；
- stateful continuation ledger、hosted/custom tool 或 opaque item 转换；
- 引入 Provider 名称分支修补 renderer；
- 同时开展异构 Provider onboarding。

## 实现切片

1. Chat request → Bridge IR；
2. Bridge IR → Responses request；
3. Responses final → Chat final；
4. Responses SSE → Chat chunks；
5. status/finish reason mapping；
6. Hermes Chat → Responses-only Provider E2E。

## 测试条目

| ID | 测试 |
|---|---|
| C4-01 | system/developer 组合和 role sequence |
| C4-02 | 单个/并行 tool calls 与 result replay |
| C4-03 | multi-item output 和顺序 |
| C4-04 | item done 与 response terminal 分离 |
| C4-05 | usage-only final、refusal、incomplete、failed 和 cancel |
| C4-06 | unknown Responses event |
| C4-07 | stateful continuation 明确拒绝或绑定 ledger |
| C4-08 | Hermes 多轮 tool loop |

## 退出条件

- Hermes Chat 在 Responses Provider 上完成最小 tool loop；
- tool call/output identity 稳定；
- item/response 状态不会被压缩成错误的成功；
- 无法表达的 built-in/opaque item 明确拒绝；
- renderer 不依赖 Provider 名称分支。

## 关联模块

- [M05 Protocol Bridge](../modules/05-protocol-bridge.md)
