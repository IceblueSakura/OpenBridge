# C3 Responses → Chat Bridge

## 阶段目标

让 Codex 的 Responses HTTP/SSE 请求通过 Chat-only Provider 完成最小文本和 function-tool loop。

## 当前状态

`Blocked by C2`。已有设计和研究，尚无运行时代码；在 Provider 聚合核心接受前不得进入当前实施计划。

## 进入条件

- C2 已 `Accepted`；
- Native Path、RoutePlan、capability、fallback 和 state affinity 已有稳定回归证据；
- Codex 的固定 Responses corpus 与一个授权 Chat-only Provider 环境可重跑。

## 首版范围

- text；
- function tool schema/call/result；
- usage；
- stream terminal；
- 不支持 stateful continuation ledger；
- hosted tool、resource/background 和 opaque item 默认拒绝。

具体的 Agent/tool 边界、identity、state owner、stream lifecycle 和 future ledger 门见[Agent Loop 兼容与 Bridge 状态契约](../design/agent-loop-bridge-contract.md)。

## 非目标

- Chat → Responses 反向 bridge；
- stateful continuation ledger、历史猜测恢复或跨 deployment state；
- hosted/custom tool、background/resource 和 opaque reasoning 转换；
- 修改 Native Path 使其经过 Bridge IR；
- 因无法表达的语义而扩大首版 bridge 承诺。

## 实现切片

1. Responses request → Bridge IR；
2. Bridge IR → Chat request；
3. Chat final → Responses final；
4. Chat SSE → Responses SSE state machine；
5. capability/reject matrix；
6. Codex → Chat-only Provider E2E。

## 测试条目

| ID | 测试 |
|---|---|
| C3-01 | text stream/non-stream |
| C3-02 | 单个和并行 tool calls |
| C3-03 | arguments 任意分片、late/empty id/name |
| C3-04 | `call_id`、tool index 和 output index 保持稳定 |
| C3-05 | tool result 缺失上下文时明确拒绝 |
| C3-06 | usage-only final、finish reason、error、EOF 和 cancel |
| C3-07 | hosted tool、opaque continuation/resource 在调用前拒绝 |
| C3-08 | bridge re-entry guard |

## 退出条件

- Codex 通过 Chat-only Provider 完成最小多轮 tool loop；
- 并行调用身份和输出顺序稳定；
- 不支持语义不被静默删除；
- 不向 Codex 注入未知或伪造终态；
- Native Path 的字段保真与性能不受影响。

## 关联模块

- [M05 Protocol Bridge](../modules/05-protocol-bridge.md)
