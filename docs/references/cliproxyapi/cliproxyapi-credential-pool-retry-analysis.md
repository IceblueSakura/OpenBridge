# CLIProxyAPI credential retry 与 cooldown 调研

## 证据范围

- 固定快照：`router-for-me/CLIProxyAPI` commit `bc71c77f5cc42f3fbe1bf040cf14d4f166894835`，2026-08-02
- 阅读入口：`config.example.yaml` 与 authentication/cooldown 文档
- 本文只记录 CLIProxyAPI 的 attempt、credential rotation 与 cooldown 行为。

## 观察事实

- `request-retry`、`max-retry-credentials`、`max-retry-interval` 与 cooldown 开关分别配置，说明请求 attempt、可尝试
  credential 数量和跨请求健康状态是不同预算。
- 默认 routing 策略是 round-robin；quota failure 会冷却当前 credential，并在 cooldown 到期后重新加入 rotation。
- 配置示例把 `403/408/500/502/503/504` 也列入 request retry，并支持 cooldown 持久化。
- 这些 status 和持久化选择与 CLIProxyAPI 的多账号/OAuth 产品形状相关，不是通用 API-key 语义。

## 适用边界

- pool 大小与单请求 attempt 上限是两个独立维度。
- cooldown 是跨请求的健康状态，不等同于当前请求等待。
- 账号轮换、余额与订阅管理不属于 HTTP retry 标准。

## 一手资料

- [
  `config.example.yaml`](https://github.com/router-for-me/CLIProxyAPI/blob/bc71c77f5cc42f3fbe1bf040cf14d4f166894835/config.example.yaml)
- [Authentication and cooldown](https://router-for-me-cliproxyapi.mintlify.app/concepts/authentication)

