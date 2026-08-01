# 当前开发焦点

## 状态

**目标注册表与 Native RoutePlan 结构迁移已完成，当前无活动焦点。** 生产代码已使用
`RealModel + UpstreamTarget + NativeOffering + ServingRoute + PublicModel`，请求路径已拆为
`RequestProfile + RoutePlan`。本次按要求没有运行测试，只完成格式化和所有 target 的编译检查；
行为回归验收仍是下一次继续工作的第一道门。

已完成边界：

- 删除 Provider/Model `routes.toml` 和 reload；
- 以 `BootstrapPolicy + compiled_definition()` 构建不可变 `RegistrySnapshot`；
- 将 OpenAI/Meituan descriptor、字段转换、认证、响应、错误和 discovery 行为集中到独立 Provider 文件；
- 以 typed definition 维护 Real Model、target、Offering、route、reasoning level 和 capability；
- Provider credential binding 已下沉到 target，同一 target 的 Chat/Responses 能力和限制相互独立；
- Probe CLI 已改为 `--target` 并按协议选择 Offering；
- 迁移测试与文档，不保留旧 schema 兼容入口。

本次 `cargo fmt --all` 与 `cargo check --locked --tests` 已通过；没有运行 `cargo test`、Clippy、SDK、
真实 Provider 或负载验证。下一项工作开始前应先补做回归门，再从 M5、M6 或按需 M7 建立独立焦点。

## 关联文档

- [代码注册表与路由](configuration-and-routing.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [当前代码架构](../implementation-status/current-architecture.md)
- [架构迁移总计划](registry-architecture-migration.md)
