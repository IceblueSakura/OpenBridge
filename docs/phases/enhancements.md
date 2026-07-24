# E1+ 核心后增强

增强不自动进入排期，也不阻塞 C0–C6。

增强项始终为 `Deferred`，不得出现在核心阶段实施计划中。只有核心阶段链完成或用户明确调整产品优先级、对应增强需求已收敛、且当前 `Active` 阶段已关闭时，才能选择其中一个增强建立新的单阶段计划。

| 阶段 | 功能 | 最低启动条件 |
|---|---|---|
| E1 | Usage/成本记录 | usage 归属、隐私字段和失败记录已定义 |
| E2 | 高级健康观测与自适应路由 | C2 最小 cooldown/retry 已验收；不破坏 state affinity 和首输出前 fallback |
| E3 | Provider-hosted tool facade | 有真实 Provider corpus、citation/result contract |
| E4 | 本地/MCP Tool Bridge | sandbox、allowlist、timeout、输出限制明确 |
| E5 | 可选 OAuth | mock issuer 通过，官方契约和条款 preflight 明确 |
| E6 | 简单管理 UI | 不持有 secret，不演化为多租户控制面 |

每个增强必须独立定义：

- 用户价值；
- 功能边界；
- 数据与安全边界；
- 实现依赖；
- 测试；
- 退出条件；
- 不支持项。

执行过程中发现的增强需求只允许补充上述需求边界，不得自动拆成新的 phase 或追加到当前计划。

关联模块：[M07 工具与增强](../modules/07-tools-and-enhancements.md)。
