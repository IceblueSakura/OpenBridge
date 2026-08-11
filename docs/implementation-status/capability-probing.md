# 上游模型发现与基础 API 探测

## 当前行为

`openbridge-probe` 是管理员显式运行的上线前基础观察工具。它只接受已注册且已启用的 `--target <id>` 与闭集 selector，
不接受 URL、model、header、credential 或任意 body 覆盖，也不修改 `RuntimeRegistry`、capability、Route、cursor 或 cooldown。

可选观察为 `--list-models`、`--chat`、`--responses`、`--embeddings` 和 `--all`。没有 selector 等同 `--all`，但只检查选定
Target 已注册的 operation，不遍历其他 Target 或 credential member。

| 观察项 | 固定请求与最低成功形状 |
|---|---|
| Models | Provider 固定 path/envelope，提取 ID 并报告注册 upstream model 是否存在 |
| Chat | 无工具最小非流式文本；成功 JSON 含非空 `choices[]` |
| Responses | 无工具最小文本；普通 Target 要求 response JSON，ChatGPT 使用固定 stream profile 和合法完成终态 |
| Embeddings | 固定字符串；成功 JSON 是匹配模型、单 embedding item 和 usage 的 list envelope |

结果只分为 `supported`、`unsupported`、`unknown`。未注册 operation 或明确 404/405/501 为 `unsupported`；认证、限流、其他 HTTP、
transport、超限、JSON/SSE/shape 错误为 `unknown`。一次 supported 只证明当时 Target、账号、网络与 payload；unknown 不证明永久不可用。

API-key Target 使用选定 pool 的首个 member。ChatGPT 只从选定 OpenBridge-owned auth file 的 manager 借用 lease；不打开未选中
文件，不读取本机 Agent/Codex identity。Report 不含 credential、request/response body。真实 probe 可能消耗额度或触发 refresh，
不属于默认测试基线。

## 所有权与确定性证据

实现位于 [`src/probe.rs`](../../src/probe.rs)、[`src/probe/`](../../src/probe/)和
[`src/bin/openbridge-probe.rs`](../../src/bin/openbridge-probe.rs)。probe 单元测试与 binary tests 使用 synthetic transport/bundle
覆盖 selector、operation、shape、limit、ChatGPT SSE 与保守分类。

## 未证明范围

Probe 不证明 function/custom/hosted tool、reasoning、structured output、媒体、state、Bridge、retry/fallback、SDK/Agent、模型质量、
配额、吞吐、负载或长期运行；也不根据结果修改代码注册事实。

## 相关文档

- [实施现状目录](README.md)
- [当前代码架构](current-architecture.md)
- [Provider 状态](providers/README.md)
