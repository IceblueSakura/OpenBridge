# Kimi CN API 协议入口

## 来源与范围

- 官方 [API 概述](https://platform.kimi.com/docs/api/overview)，复核日期：2026-08-09；
- 官方 [Chat Completions API](https://platform.kimi.com/docs/api/chat)，复核日期：2026-08-09。

本页只记录 Kimi 中国开放平台的公开协议事实，不记录 OpenBridge 实现或私有 credential。

## 已确认事实

- 中国开放平台固定服务地址为 `https://api.moonshot.cn`，OpenAI-compatible SDK base URL 为
  `https://api.moonshot.cn/v1`。
- 文本生成使用 `POST /v1/chat/completions` 和 Bearer API key；Kimi-specific 字段不改变该固定 endpoint 或认证边界。
- 官方兼容说明是请求/响应格式兼容，不表示每个 OpenAI 参数在每个 Kimi 模型上都可修改；模型级约束以
  [Kimi 模型参数](models.md)为准。

## 证据边界

文档事实不证明某个账户当前有模型权限，也不证明 Responses Native、负载、长期运行或未来版本行为。
