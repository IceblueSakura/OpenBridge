# new-api 调研索引

本目录记录 new-api 的产品边界、请求转发链、协议转换、渠道路由、计费和运维机制。许可证见
[GNU AGPL v3](https://github.com/QuantumNous/new-api/blob/2d8e50bf36e94200b809dfb39e73624ec48b1e23/LICENSE)。

固定源码快照为 `QuantumNous/new-api` @ `2d8e50bf36e94200b809dfb39e73624ec48b1e23`，本地 checkout
复核于 2026-08-24。除文中明确记录的 focused tests 外，本目录不证明真实 Provider、负载或生产部署表现。

| 主题 | 文档 |
|---|---|
| 产品定位、模块边界和请求主链 | [架构与产品形状](new-api-architecture-analysis.md) |
| Chat、Responses、Claude、Gemini 请求转换 | [请求转换系统](new-api-request-conversion-analysis.md) |
| 渠道选择、retry、计费、观测和后台任务 | [路由、计费与运维](new-api-routing-billing-operations-analysis.md) |

new-api 是控制面与数据面同进程的多租户 AI API 聚合平台。其 deployment、用户组、余额、支付、动态配置和管理后台属于
产品控制面，不自动形成通用 gateway contract；其转换器的兼容行为、质量标签和测试通过也不替代官方协议或真实 Provider 验证。
依赖某一转换、retry、billing 或动态配置行为前，必须回到固定 commit 的叶文档和对应源码重新核对。
