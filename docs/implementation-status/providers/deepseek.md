# DeepSeek Provider 状态

## 当前注册

- Provider family：`deepseek`；可信 Base URL：`https://api.deepseek.com`（不带 `/v1`）；
- credential pool：`deepseek-primary`，仅允许 API key；
- 固定 2 个 Target：`deepseek-v4-pro`（Chat-only）、`deepseek-v4-flash`（Chat + Responses Native）；
- Chat 端点只公开 `json_object` structured output（`response_format.type=json_schema` 被上游 400 拒绝）；
- Responses 端点公开 `JsonObjectAndJsonSchema(StrictSupported)`（2026-08-11 实测 `strict:true` 被接受且输出符合 schema）；
- 两端工具 `strict_schema` 均保持 `false`：本轮未实测 DeepSeek 工具 strict。

## 真实验证（2026-08-11）

使用当前私有 credential 直连 `https://api.deepseek.com`：

- `GET /models`：HTTP 200，返回 `deepseek-v4-flash`、`deepseek-v4-pro`；
- Chat `response_format:{type:"json_object"}`：HTTP 200，正常 JSON 输出；
- Chat `response_format:{type:"json_schema", strict:true}` 与 `strict:false`：均 HTTP 400，
  `"This response_format type is unavailable now"`（`json_schema` 整体不可用，与 strict 值无关）；
- Responses `text.format:{type:"json_schema", schema, strict:true}`：HTTP 200，输出严格符合 schema
  （`{"answer":"Paris"}`，响应回显 `strict:true`）；
- Responses `text.format:{type:"json_object"}`：HTTP 200；
- `/beta` 前缀（`https://api.deepseek.com/beta`）：Chat/Responses 行为与根路径完全一致；仅
  `GET /beta/models` 为 404，即 beta 是部分路由别名，不是独立特性开关。

## 证据边界

- 单账号、单区域请求，不证明其他账号/区域/未来 Provider 行为；
- 只实测 `deepseek-v4-flash`；`deepseek-v4-pro` 不绑定 Responses 端点，Chat 端与 flash 共享
  Provider ceiling（`json_object`），本轮无模型级外推；
- 工具调用 strict、SSE streaming 的 structured output、SDK/Agent 兼容和负载均未在本轮覆盖；
- 能力声明由 `src/providers/deepseek/definition.rs` 的 `definition::tests` 单元断言锁定。

## 相关文档

- [Provider 状态目录](README.md)
- [Models 接口与能力预检](../features/models-api-and-capability-preflight.md)
