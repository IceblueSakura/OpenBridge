# OpenAI SDK 原生 Chat/Responses tool-loop 验证

## 状态

Confirmed（限于 SDK-first 假设）

## 研究问题与假设

- Research question：OpenBridge 的当前 Native Path 能否让指定版本的 OpenAI Python/Node SDK 消费 Chat Completions 和 Responses 的文本、流式、单/并行 function-tool 往返与确定性错误 fixture？
- Hypothesis：当上下游均为 OpenAI wire protocol 时，OpenBridge 仅改写 `model` 并保留 JSON/SSE/HTTP error 语义，足以让 SDK 解码文本、tool call、tool result、分片 arguments 和 429 error。
- Affected decision：作为 SDK-first wire regression；不替代 Codex CLI custom Provider E2E、真实 Provider corpus 或任何已声明 Hermes 兼容的 E2E。

## 环境

- OpenBridge commit：`a3c6e0183475f854bdfdff53a3d6a97904557f47`（运行测试前的 `HEAD`；工作区包含未提交的 M1 变更）。
- Client/version/commit：OpenAI Python `2.46.0`；OpenAI Node `6.48.0`。
- Provider/model/API version：本地 mock upstream；`upstream-model`；不访问真实 Provider。
- OS/runtime：Windows；Rust `cargo test`；Python SDK 由 `uv` 临时安装；Node SDK 由 npm 临时安装。
- Configuration snapshot：测试内 loopback OpenBridge 配置，public alias 为 `public-model`，上游模型为 `upstream-model`；credential 使用测试固定值，不写入配置或 artifact。

## Fixture 与步骤

1. 运行 `tests/sdk_compatibility.rs`，它启动 loopback OpenBridge 和 mock `UpstreamTransport`。
2. OpenAI Python/Node SDK 调用 `/v1/chat/completions` 与 `/v1/responses`。
3. 断言 SDK 可消费：
   - Chat/Responses 的 stream 与 non-stream 文本；
   - Chat 单/并行 function call、同一 `tool_call_id` 的 tool result 回传、流式 arguments 分片；
   - Responses 单/并行 `function_call`、同一 `call_id` 的 `function_call_output` 回传、流式 arguments delta/done、item done 与 response completed；
   - mock upstream 的 HTTP 429 JSON error，可被 SDK 解码为带 `status_code`/`code` 的异常。

Artifacts：

- 原始或脱敏 request：测试中的 SDK 请求构造，见 `tests/sdk/openai_python_compat.py`、`tests/sdk/openai_node_compat.cjs`。
- 原始或脱敏 response/SSE bytes：测试中的 mock fixture，见 `tests/sdk_compatibility.rs`。
- 客户端观察事件：两个 SDK 脚本的断言。
- 测试/脚本：

  ```powershell
  $env:OPENBRIDGE_NPM='C:\Program Files\nodejs\npm.cmd'
  $env:OPENBRIDGE_NODE='C:\Program Files\nodejs\node.exe'
  cargo test --locked --test sdk_compatibility -- --ignored
  ```

## 预期结果

Python 和 Node SDK 均成功解码上述 loopback fixture；代理不读取真实 Provider credential，不连接真实 Provider。

## 观察结果

2026-07-24：命令退出成功。一个 ignored integration test 通过；Python `2.46.0` 与 Node `6.48.0` 均完成文本、单/并行 function-tool fixture 以及 mock 429 error 解码。

首次尝试使用默认 npm shim 失败；明确使用系统 `C:\Program Files\nodejs\npm.cmd` 后通过。该问题是本机 npm 路径选择，不是 OpenBridge wire 行为。

## 这证明什么

- 当前 Native Path 的 loopback Chat/Responses JSON/SSE 可被指定 OpenAI SDK 版本消费。
- SDK 可观察到单/并行 tool call/result identity，以及 Chat/Responses 流式 arguments 分片。
- mock upstream 的 429 JSON error 能经 Native Path 保留为对应 SDK 的 status/code 异常。
- 测试夹具覆盖的 SSE framing 仍能在 SDK 侧完成解码。

## 这不证明什么

- 不证明真实 OpenAI 或其他 Provider 会产生等价 JSON/SSE，或其真实模型支持所用工具循环。
- 不证明 Codex、Hermes 或完整 Agent runtime 的配置、工具执行、并行调度、approval、cancel、continuation 或 WebSocket 行为。
- 不证明未知 Provider event、真实 Provider error、真实 client cancel 或跨 deployment state affinity 的 SDK E2E。
- 不证明 Chat ↔ Responses bridge、第二 Provider Family 或异构 Provider 兼容。

## 结论

- Result：Confirmed（仅限 SDK-first hypothesis）。
- Decision impact：SDK-first 证据已可重跑；它只证明本实验环境下的 SDK 行为，不能据此推断 Codex CLI、真实 Provider 或完整兼容性。
- Required follow-up：保留 EOF/partial-stream/unknown event/cancel 的 Rust contract evidence；每次需要作为证据时记录 SDK/Codex CLI 的滚动运行环境；若交付宣称 Hermes 兼容，再记录其运行版本并补 E2E。
- Revalidation trigger：升级 OpenAI Python/Node SDK、修改 Native Path/SSE decoder、修改 OpenAI-compatible adapter，或切换 Node package manager/runtime。
