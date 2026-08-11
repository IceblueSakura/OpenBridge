# Endpoint 与出站边界

## 状态

本文是[配置与凭证域](README.md)的 Endpoint 模块：定义上游 Endpoint 的来源、URL 校验与出站边界。
其他模块见[配置与凭证域](README.md)导航。

## 1. Endpoint 与出站边界

Endpoint 只来自代码注册的 Provider 实例。每个实例只有一个 BaseURL；Provider adapter 对每个受支持 operation 只提供一条静态相对
path，因此一个实例对每个 operation 至多形成一份上游 URL。Registry builder 必须拒绝：

- 非 HTTPS endpoint；
- 缺少 host；
- userinfo、query 或 fragment；
- 双斜线、空 segment、`.`、`..`；
- 编码斜线或不受限字符构成的 path prefix。

共享 transport 只能把 Provider adapter 生成的相对 path 追加到已校验 endpoint base，且禁用 redirect。业务请求、adapter 和
credential 均不能替换 endpoint origin。

## 关联文档

- [配置与凭证域导航](README.md)
- [所有权划分与代码注册表](ownership-and-registry.md)
- [生命周期](lifecycle.md)
- [实施现状](../../implementation-status/README.md)
