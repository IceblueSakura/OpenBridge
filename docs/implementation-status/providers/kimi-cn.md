# Kimi CN Provider 状态

## 当前注册

- Provider family：`kimi-cn`；
- 可信 Base URL：`https://api.moonshot.cn`；
- credential pool：`kimi-primary`，仅允许 API key；
- Public Model：`kimi-k3`；
- 上游模型：`kimi-k3`；
- 上游接口：只注册 Chat Completions；
- Route：一个 Chat Native Route，adapter 相对路径为 `/v1/chat/completions`；Public Model 编译器在缺少 Responses Native 时自动
  补充一个 Responses-via-Chat Bridge Route；
- 当前公开契约：Chat/Responses 的文本与 streaming 基线；没有 Responses Native、Embeddings 或动态 endpoint/credential，Bridge 仍受
  完整 preflight 的共同语义和能力边界约束。

## 证据边界

`tests/example_config.rs` 中的 `kimi_cn_k3_compiles_with_native_chat_and_auto_responses_bridge` 已验证 Provider、pool、endpoint、三层
模型身份、Target、Public Model、Chat Native/Responses Bridge Route、本地两协议规划以及 adapter 的相对请求路径和上游 model 替换。
`tests/provider_contract.rs` 同时验证 Kimi CN 使用 API-key、仅声明 Chat Native 上游基线，并保持相对 URI 与 credential header 的
Provider 边界。

这些是确定性注册表和进程内 adapter 证据，不证明真实 Moonshot 账号权限、模型可用性、网络、配额、外部 SDK、负载或长期运行行为。
本 checkout 尚未执行 Kimi CN 真实 Provider 请求或 Models probe。
