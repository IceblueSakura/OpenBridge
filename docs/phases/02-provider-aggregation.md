# C2 Provider 聚合核心

## 阶段目标

让多个 Provider Family 和 deployment 通过稳定 alias 聚合，并保持确定性路由、安全配置、capability gate、fallback 与 state affinity。

## 当前状态

`In progress`。snapshot、alias、ordered candidates、capability gate、首输出前 fallback 和基础 state protection 已实现；当前只有一个 OpenAI Family。

## 实现范围

- Provider Family 与 Deployment 分离；
- owner-configured endpoint 和 credential reference；
- public alias 与有序 candidates；
- capability filtering；
- 原子配置 reload；
- 首输出前 retry/fallback；
- provider-bound continuation affinity；
- 第二 Provider Family；
- Provider conformance suite。

## 测试条目

| ID | 测试 |
|---|---|
| C2-01 | 相同 snapshot + request 产生相同 candidate set |
| C2-02 | deployment 不能提升 Family capability |
| C2-03 | 下游不能改变 origin、credential、path 和认证 header |
| C2-04 | reload 成功原子切换，失败保留旧 snapshot |
| C2-05 | 429/5xx/timeout/连接错误仅在首输出前有界 fallback |
| C2-06 | 输出后不 fallback、不拼接 stream |
| C2-07 | `previous_response_id` 和 tool continuation 不跨 deployment |
| C2-08 | 两个 Family 通过同一 conformance suite |

## 退出条件

- 至少两个 Provider Family；
- Generic compatible endpoint 不需要复制 transport；
- route 确定且客户端不可提升权限；
- fallback 与 state affinity 通过故障注入；
- conformance suite 可用于新 Family onboarding。

## 关联模块

- [M02 配置与路由](../modules/02-configuration-and-routing.md)
- [M03 Provider Adapter](../modules/03-provider-adapters.md)
- [M06 安全与凭证](../modules/06-security-and-credentials.md)
