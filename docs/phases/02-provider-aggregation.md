# C2 Provider 聚合核心

## 阶段目标

让多个 Provider Family 和 deployment 通过稳定 alias 聚合，并保持确定性路由、安全配置、capability gate、有限 retry、被动 cooldown、fallback、错误传播与 state affinity。

## 当前状态

`Blocked by C1`。snapshot、alias、ordered candidates、capability gate、首输出前 fallback 和基础 state protection 已有原型实现；这些事实不代表本阶段已经启动，当前仍只有一个 OpenAI Family。

## 进入条件

- C1 已 `Accepted`；
- 双 Native Path corpus 可作为 Provider 抽象调整的回归基线；
- Native Path 绕过 Bridge IR、state affinity 和首输出前 fallback 边界未被 C1 反证。

## 实现范围

- Provider Family 与 Deployment 分离；
- owner-configured endpoint 和 credential reference；
- public alias 与有序 candidates；
- capability filtering；
- 原子配置 reload；
- deployment 级 availability/cooldown overlay；
- 次数、等待和总耗时有上限的首输出前 retry/fallback；
- 最终 Provider 错误和 rate-limit hint 的安全传播；
- provider-bound continuation affinity；
- 第二 Provider Family；
- Provider conformance suite。

## 非目标

- 实现 Responses → Chat 或 Chat → Responses bridge；
- 引入非 OpenAI wire dialect；异构反证属于 C5；
- 实现复杂动态权重、主动健康系统、多账号池或多租户控制面；
- 用配置声明提升 Provider Family 未经实现证明的 capability。

## 测试条目

| ID | 测试 |
|---|---|
| C2-01 | 相同 snapshot + request 产生相同 candidate set |
| C2-02 | deployment 不能提升 Family capability |
| C2-03 | 下游不能改变 origin、credential、path 和认证 header |
| C2-04 | reload 成功原子切换，失败保留旧 snapshot |
| C2-05 | 429/临时 5xx/连接错误按分类进入有界 retry/fallback；timeout 或重复安全性不明时停止 |
| C2-06 | 输出后不 fallback、不拼接 stream |
| C2-07 | `previous_response_id` 和 tool continuation 不跨 deployment |
| C2-08 | 两个 Family 通过同一 conformance suite |
| C2-09 | 429 + `Retry-After` 建立 deployment cooldown；新无状态请求跳过并在到期后恢复 |
| C2-10 | 全部 candidate cooling down 时返回稳定 429 code 和最早有效 `Retry-After` |
| C2-11 | 最终错误保留 allowlist 内的 status、error fields、Provider request id 和 rate-limit headers |
| C2-12 | 并发 429、cancel during backoff、reload identity 变化和 cooldown registry 上限 |

## 退出条件

- 至少两个 Provider Family；
- Generic compatible endpoint 不需要复制 transport；
- route 确定且客户端不可提升权限；
- retry/cooldown/fallback、最终错误传播与 state affinity 通过故障注入；
- conformance suite 可用于新 Family onboarding。

## 关联模块

- [M02 配置与路由](../modules/02-configuration-and-routing.md)
- [M03 Provider Adapter](../modules/03-provider-adapters.md)
- [M06 安全与凭证](../modules/06-security-and-credentials.md)
- [Provider 限流、冷却、重试与错误传播需求](../requirements/provider-resilience.md)
