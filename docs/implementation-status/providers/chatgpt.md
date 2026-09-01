# ChatGPT 接入进度与边界

注册与能力事实见 `src/providers/chatgpt/`；当前接线见[映射](../model-provider-mapping.md)。
OAuth credential 生命周期合同见 [configuration/oauth-chatgpt.md](../../functional-requirements/configuration/oauth-chatgpt.md)。

## 当前边界

- WebSocket、Batch、Embeddings、hosted/custom tool、MCP、真实图片输入、background/stateful response、完整 Agent loop、
  多账户轮换、外部 SDK、负载和长期 refresh 稳定性未实现或未证明。
- 真实登录、refresh 与 401 recovery 的长期稳定性只有确定性测试覆盖，未形成真实账号长期运行验收。

## 验证与证据

- 启动期 OAuth bundle 校验、登录与刷新由 `tests/oauth2_login_cli.rs` 与 startup contract 覆盖；无真实账号长期运行记录。

## 代码 owner

`src/providers/chatgpt/`、`src/oauth2_credentials/`。
