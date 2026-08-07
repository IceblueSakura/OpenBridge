# 功能：Chat Completions 与 Responses Native 转发

## 状态

**已完成（当前 checkout）。** 网关可以对已注册且通过 Public Model 预检的 Chat/Responses 请求执行 Native JSON 或 SSE 转发，并保留目标协议
允许的原生语义。

## 已完成内容

- `POST /v1/chat/completions` 和 `POST /v1/responses` 支持当前声明范围内的非流式 JSON 与 streaming SSE。
- Native Route 保留下游 canonical request；Provider adapter 在 egress 阶段绑定固定 upstream model、相对 path、普通固定 header 和
  purpose-bound authentication。
- 已声明的 reasoning level 可以按 Provider 规则映射到 wire value；未知或未声明 level 在 egress 前拒绝。
- 当前 Native surface 包括 OpenAI `gpt-5.6-sol`、LongCat `LongCat-2.0`、DeepSeek Chat、OpenRouter 的
  `deepseek-v4-flash` Chat/无状态 Responses，以及 Xiaomi MiMo 的 Chat/Responses。
- `mimo-v2.5` 的两个同协议 Native surface 还支持固定 typed contract 内的 URL/Base64 图片输入；具体边界和真实 Provider 证据由
  [Native 图片专题](native-image-input.md)记录。
- OpenRouter 的 `store: true`、非空 `previous_response_id` 和 `background: true` 等未声明状态语义在 egress 前拒绝；DeepSeek V4 当前只
  注册 Chat Native API。
- 上游 safe response headers、SSE framing、terminal、EOF-before-terminal 和 body failure 在统一 ingress/transport 边界处理。

## 实现边界

- 请求入口位于 [`src/ingress/`](../../../src/ingress/)，请求分析/规划位于 [`src/pipeline/`](../../../src/pipeline/)，Provider adapter 位于
  [`src/provider/adapter.rs`](../../../src/provider/adapter.rs)，共享发送边界位于 [`src/transport/upstream.rs`](../../../src/transport/upstream.rs)。
- Native Route 的额外能力不会扩大 Public Model；它必须先通过公共 interface preflight。
- 这不是外部 OpenAI SDK、Codex/Hermes Agent、真实 Provider、负载或长期运行兼容性声明。

## 验证证据

- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 Native JSON/SSE、错误、header 和响应收口。
- [`tests/sse_contract.rs`](../../../tests/sse_contract.rs) 覆盖 SSE framing、terminal、EOF 和错误边界。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 与 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider wire、认证和安全出站。
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖公共契约与候选规划。

## 相关文档

- [功能需求：网关 API 与客户端兼容](../../functional-requirements/gateway-api-compatibility.md)
- [协议 Bridge](protocol-bridge.md)
- [`mimo-v2.5` Native 图片输入](native-image-input.md)
- [重试、fallback、cooldown 与取消](resilience-retry-fallback-and-cancellation.md)
- [当前代码架构](../current-architecture.md)
