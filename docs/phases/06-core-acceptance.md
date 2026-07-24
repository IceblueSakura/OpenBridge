# C6 核心接受

## 阶段目标

将已验证能力收敛为可发布、可复现、可安全部署、可 onboarding 新 Provider 的核心版本。

## 当前状态

`Blocked`，依赖 C1–C5 Accepted。

## 接受范围

- 固定 Codex/Hermes 兼容矩阵；
- 至少三个 Provider archetype；
- Native Path、双向 Bridge、capability、state 和 fallback；
- loopback 与非 loopback 安全基线；
- timeout、backpressure、cancel、shutdown 和 reload rollback；
- Provider onboarding、配置、排障、安全部署、发布和回滚文档；
- 配置 schema 与版本策略；
- release artifact、checksum、smoke 和 rollback 演练。

## 测试条目

| ID | 测试 |
|---|---|
| C6-01 | C1–C5 gate review 全部 Accepted |
| C6-02 | secret scan、日志脱敏、origin/header/redirect policy |
| C6-03 | 非 loopback token + TLS/可信反向代理配置 |
| C6-04 | body/event/buffer/timeout/retry 资源边界 |
| C6-05 | graceful shutdown、cancel、reload failure rollback |
| C6-06 | native/bridge TTFT、延迟和内存回归基线 |
| C6-07 | clean environment smoke 与 Provider onboarding |
| C6-08 | release、upgrade 和 rollback 演练 |

## 退出条件

- 所有核心需求有实现、测试和兼容性证据；
- 目标客户端和 Provider corpus 可复现；
- 新 Provider onboarding 不修改核心 router；
- 已知限制进入兼容矩阵；
- release candidate 和回滚方案完成评审。

## 关联模块

- [全部功能模块](../modules/README.md)
