# C0 范围与客户端契约

## 阶段目标

固定首版产品范围、目标客户端版本、配置和 wire contract，为后续实现提供可复现的输入。

## 当前状态

`Active`，是当前唯一允许存在详细实施计划的阶段。产品范围和客户端契约已成文，固定版本、真实 corpus 和 gate review 尚未完成。

## 进入条件

- 单用户、单服务的产品定位已明确；
- Codex 与 Hermes 已确定为首批目标客户端；
- 本阶段可以只通过需求、配置、fixture 和实验记录完成，不要求新增业务能力。

## 工作范围

- 固定一个 Codex 版本、平台和 custom Provider 配置；
- 固定一个 Hermes Agent 版本、平台和 Chat 配置；
- Codex 使用 Responses HTTP/SSE，显式关闭 WebSocket；
- Hermes 以 Chat 为 P0，Responses 为后续对照；
- 建立成功、错误、EOF、partial stream、cancel 和 tool-loop corpus；
- 每个 fixture 记录环境、配置、原始/脱敏 wire、预期结果和证明边界。

## 非目标

- 修复或扩展 Native Path 运行时代码；
- 实现第二 Provider Family、Protocol Bridge 或 continuation ledger；
- 启动 OAuth、Hosted Tool/MCP、usage 或 UI；
- 用 SDK/mock 证据替代真实客户端和真实 Provider corpus；
- 因发现后续实现事项而新增或展开新的 phase。

## 测试条目

| ID | 测试 |
|---|---|
| C0-01 | Codex custom Provider `base_url`、token、wire API 和 `supports_websockets = false` |
| C0-02 | Codex 实际 transport 诊断确认 HTTP/SSE |
| C0-03 | Hermes Chat Provider 配置与 endpoint |
| C0-04 | 两个客户端的请求、成功 SSE 和错误 SSE corpus |
| C0-05 | cancel、EOF、partial stream 和 unknown field/event corpus |
| C0-06 | fixture 脱敏、版本、配置和重跑命令完整 |

## 退出条件

- 核心范围和非目标明确；
- 两个目标客户端版本与配置固定；
- 两个 P0 Native Path 的 wire contract 有可复现 corpus；
- WebSocket 不构成首版隐式依赖；
- C0 review 逐条链接测试和 fixture。

## 关联模块

- [M00 产品边界](../modules/00-product-scope.md)
- [M01 客户端 API](../modules/01-client-api.md)
