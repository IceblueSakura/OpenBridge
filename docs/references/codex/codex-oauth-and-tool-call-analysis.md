# Codex 浏览器 OAuth 与工具调用源码调研

## 状态与证据

- 原始逐行快照：`openai/codex` commit `0fb559f0f6e231a88ac02ea002d3ecd248e2b515`
- 当前模块级复核：commit `ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff`，2026-08-01
- 阅读范围：`codex-rs/login` 的浏览器 OAuth、refresh、请求认证，以及 `codex-rs/core` 的 Responses tool invocation
- 未读取、输出或复制本地 credential、auth cache、client identifier 或 token。

本文保留旧快照的 loopback browser flow 与 tool invocation 证据。当前设备登录另见[Codex 设备登录与 token 刷新](codex-device-auth-token-refresh-analysis.md)，SSE/event 另见[Codex Responses SSE 与工具生命周期](codex-sse-and-tool-lifecycle-analysis.md)。

## 1. 浏览器 OAuth 登录

旧快照中的 login server 使用 authorization-code + PKCE：

1. 生成 PKCE verifier/challenge 与随机 `state`；
2. 在 loopback listener 上等待 callback；
3. 打开授权 URL；
4. callback 校验 `state` 与授权结果；
5. 使用 authorization code 和 verifier 交换 token；
6. 检查允许的 workspace；
7. 将认证状态写入配置的 credential-store backend。

该流程属于本地 CLI 产品。loopback redirect、client registration、scope、token endpoint 和 workspace policy 都是 Codex 当前 client contract 的组成部分。

## 2. Credential storage 与 refresh

Codex auth state 可能包含 API key、id/access/refresh token、刷新时间、agent identity 或 personal access token。实际 backend 可以是文件或 OS credential store。

refresh 路径具有以下性质：

- 进程内 refresh lock；
- 取得锁后重新加载当前 credential；
- 若存储状态已经变化则跳过重复 refresh；
- API key/PAT 与 ChatGPT OAuth 分流；
- refresh response 中的新 access/id/refresh token 和刷新时间一起写回；
- 401 recovery 先 reload，再 refresh，状态机有界结束。

请求认证还绑定 account、ChatGPT user 与 workspace identity；identity 不一致时不会简单附加 bearer header。

## 3. Tool item 到 invocation

Codex core 从 Responses output item 区分 function call、custom tool call、local shell 等工具形状。`ToolRouter` 依据 tool type/name 构造对应 invocation，并保留 call identity。

function call 的关键数据包括：

- `call_id`：模型调用与后续 output 的关联键；
- item id：Responses output item 的 lifecycle identity；
- name：工具选择，不替代 call identity；
- arguments/input：可在 stream 中分片到达。

未知或禁用工具会进入明确错误/拒绝路径，而不是仅凭 name 调用任意本地函数。

## 4. Streaming arguments 与启动时机

stream consumer 会累计 arguments/input delta，并在 item lifecycle 允许时形成完整 invocation。工具可以在 response terminal 之前启动；因此 item done 与 response completed 不是同一时刻。

并行工具执行还需要：

- 每个 call 独立的 identity 与 cancellation handle；
- output 按 `call_id` 回接；
- 多个 invocation 的完成顺序不改变原 call identity；
- turn 取消时传播到仍运行的工具。

## 5. Tool output 回传

本地工具结果被编码为对应 Responses input item，并在下一轮请求中与原 `call_id` 关联。tool execution、approval、sandbox、用户提示和本地副作用都属于 Codex Agent runtime。

## 6. 证据边界

- Codex browser/device OAuth 只证明 Codex CLI 行为，不公开保证第三方 client 可复用。
- auth cache 是产品内部 credential 状态，不是跨应用交换格式。
- Codex tool runtime 不等于 Responses server 应执行客户端 function tool。
- 当前 client 能处理的 tool/event 子集不等于完整 OpenAI API 规范。
- 原始逐行快照已演进；具体修改前应使用当前 commit 重新定位。

## 相关项目文档

- [Codex 设备登录与 token 刷新](codex-device-auth-token-refresh-analysis.md)
- [Codex Responses SSE 与工具生命周期](codex-sse-and-tool-lifecycle-analysis.md)
- [Codex protocol test assets](codex-protocol-test-assets-analysis.md)
- [OAuth 四项目综合调研](../cross-project/upstream-oauth-device-code-token-refresh-analysis.md)
