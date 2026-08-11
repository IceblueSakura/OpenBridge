# Public Model 身份、生命周期与可见性

## 状态

本文是[模型与能力契约域](README.md)的身份模块：定义 Public Model 的稳定身份、生命周期状态和目录可见性规则。
其他模块见[模型与能力契约域](README.md)导航。

## 1. 身份、生命周期与可见性

- `id` 是客户端请求和资源路径使用的稳定单段标识，格式为
  `[A-Za-z0-9][A-Za-z0-9._:-]{0,127}`；包含 `/` 的上游模型名不得直接成为 Public Model id。
- `created` 是 Public Model 契约首次创建的稳定 Unix 秒，不使用进程启动时间。
- `name`、可选 `description` 和 `lifecycle` 是面向客户端的静态元数据。
- `active` 与 `deprecated` 模型仍可列出和调用；`retired` 模型对 list、retrieve 和模型请求统一表现为不可用。
- 没有任何静态可执行 Chat/Responses/Embeddings 接口的 Public Model 不进入可见目录。
- 标准列表、扩展列表、两个 retrieve 接口和请求预检必须读取同一个不可变 registry snapshot。

## 关联文档

- [模型与能力契约域导航](README.md)
- [事实所有权与公开边界](fact-ownership-and-boundary.md)
- [Models API 契约](models-api.md)
- [启动时校验](startup-validation.md)
- [当前实现总览](../../implementation-status/current-implementation.md)
