# ChatGPT 接入进度与边界

注册与能力事实见 `src/providers/chatgpt/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。
OAuth credential 生命周期合同见 [configuration/oauth-chatgpt.md](../../functional-requirements/configuration/oauth-chatgpt.md)。

## 特有接线与例外

- 通过 ChatGPT subscription backend 注册 Responses-native Targets；Public Model 关系见映射，不在本页复制模型库存。
- 已注册单张内联图片输入：仅 data URL，JPEG/PNG/GIF/WebP，20 MiB encoded / 15 MiB decoded 上限，无显式 `detail`；remote URL、多图和真实图片质量未证明。
- WebSocket、Batch、Embeddings、hosted/custom tool、MCP、background/stateful response 与完整 Agent loop 未接入；多账户轮换也不属于当前接线。
- 真实登录、refresh、401 recovery 和长期 token 稳定性只有 fake/deterministic 覆盖，没有真实账号长期运行验收。

## 验证入口

- `tests/oauth2_login_cli.rs` 与 startup contract 覆盖启动期 OAuth bundle 校验、登录和刷新语义；当前没有带日期的真实账号长期运行记录。

## 代码 owner

`src/providers/chatgpt/`、`src/oauth2_credentials/`。
