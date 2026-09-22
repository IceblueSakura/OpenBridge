# 当前开发焦点

## 阶段

**Generation Semantic Migration & Validation**

`semantic-v2` 已结束以架构文档扩展为主的阶段。当前目标不是继续横向设计，而是通过阶段性代码迁移和离线测试验证 Semantic IR 架构。

## 当前唯一主线

以 Generation 为第一个完整 vertical slice，从旧实现、测试和 Provider evidence 中提取已经验证的协议语义，迁移到：

```text
src/semantic/
src/protocol/
src/lowering/
```

并验证：

```text
Chat / Responses wire
 -> Generation IR
 -> semantic transform
 -> requirements
 -> representability
 -> Chat / Responses wire
```

## 当前顺序

1. 完善 Generation IR：tools、tool history、structured output、reasoning、resource、identity、presence。
2. 完成 Chat 与 Responses 的双向 request/response codec。
3. 将旧 bridge/IR/evidence 中的 edge cases 转为 semantic conformance tests。
4. 验证 semantic convergence、IR authority、fidelity isolation 和 representability failure。
5. 完成 legacy responsibility assessment。

## 非目标

本阶段不推进 Embedding/Image/Speech IR，不重写 Provider/Route/Credential/Execution，不设计通用 hook/plugin，不以新增 ADR 或大规模文档扩写替代代码验证。

## 完成条件

Generation 的主要支持语义能够脱离真实 Provider 和网络完成双向 codec、IR 变换、requirements projection 和 representability check，并由独立测试证明 IR 是唯一语义权威。

达到该条件后，再进入 Endpoint / Topology / Execution 迁移。
