# 功能：Chat Completions 与 Responses Native 转发

## 当前行为

- 已注册且通过 Public Model preflight 的 Chat/Responses 请求可执行非流式 JSON 或 SSE Native 转发。
- Native candidate 保留已知且固定 interface 接受的 canonical wire；Provider adapter 绑定 upstream model、相对 path、固定安全
  header 与 purpose-bound credential。未知顶层字段不透明透传。
- Upstream streaming policy 可选或 required；ChatGPT required Responses stream 可在完整 terminal 前提下有界转换为下游 JSON。
- `prompt_cache_key` 只按具体 Target/API exact-forward；`include` 按逐值 interface contract 处理，不保证输出 item 或缓存效果。
- `parallel_tool_calls`、structured output、tool choice、reasoning、普通参数 ignore/disable 与 stream usage 均按全部固定 candidate
  交集公开，不根据请求跳过较弱 candidate。
- Chat `stream_options.include_usage:true` 在 Native 保留原对象/Provider usage；`{}`/`false` 在 candidate egress 前移除。
- 所有 Responses candidate 显式使用 `store:false`；`store:true` 在 Route 前拒绝。state/continuation 受固定 issuing affinity 约束。
- Safe response headers、SSE framing/terminal、EOF-before-terminal、body error/cancel 与首次输出 commit point 在统一 ingress/transport
  边界处理。普通 stream success 必须有唯一 SSE media；ChatGPT 缺失 success Content-Type 是静态 profile 的独立例外。

## 所有权

入口与生命周期位于 [`src/ingress/`](../../../src/ingress/)，analysis/planning 位于 [`src/pipeline/`](../../../src/pipeline/)，
Provider adapter 位于 `src/provider/adapter.rs`，HTTP/SSE 发送位于 [`src/transport/`](../../../src/transport/)。图片、音频和 Bridge
分别由独立专题拥有。

## 确定性证据

- `tests/forwarding_contract.rs`：Native JSON/SSE、exact egress、参数、header、错误、takeover 和 commit point。
- `tests/sse_contract.rs`：framing、UTF-8、terminal、EOF 与取消。
- `tests/provider_contract.rs`、`tests/provider_boundary_contract.rs`：Provider wire、认证、安全出站和错误分类。
- `tests/config_contract.rs`：operation/capability/streaming policy 启动校验。

## 真实 Provider 证据

[2026-08-09 文字矩阵](../evidence/real-provider/2026-08-09-text-generation-none-high-matrix.md)与
[2026-08-10 Qwen3.6 矩阵](../evidence/real-provider/2026-08-10-qwen36-none-high-matrix.md)记录正常首选路径的 JSON/SSE
终态。DeepSeek 的直连 Chat/Responses structured-output 证据和 Provider-specific 边界见 [DeepSeek 状态](../providers/deepseek.md)；
OpenRouter/NVIDIA/Bailian/MiMo/ChatGPT 的 include、parallel、图片与模型级收窄见各自 Provider 状态页。

## 未证明范围

真实矩阵未强制后备 source，也不证明外部 OpenAI SDK、Codex/Hermes runtime、其他账号、Provider 内部并行、cache hit、计费、
负载或长期运行。

## 相关文档

- [网关 API 需求](../../functional-requirements/gateway-api/README.md)
- [Protocol Bridge](protocol-bridge.md)
- [Native 图片](native-image-input.md)
- [韧性与取消](resilience-retry-fallback-and-cancellation.md)
