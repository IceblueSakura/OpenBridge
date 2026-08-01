# 当前开发焦点

## 状态

**当前无活动焦点。** 生产代码使用
`Model + UpstreamTarget + UpstreamApi + Route + PublicModel`，请求路径使用
`RequestRequirements + RoutePlan`。最近一次记录的验证只包含格式化和所有 target 的编译检查，没有运行测试；
下一项功能开始前应先根据风险选择相应回归验证。

当前实现边界：

- 以 `BootstrapConfig + compiled_config()` 构建不可变 `RuntimeRegistry`；
- 将 OpenAI/LongCat contract、字段转换、认证、响应、错误和 discovery 行为集中到独立 Provider 文件；
- 以 typed config 维护 Model、target、Upstream API、route、reasoning level 和 capability；
- Provider credential binding 已下沉到 target，同一 target 的 Chat/Responses 能力和限制相互独立；
- Probe CLI 已改为 `--target` 并按协议选择 Upstream API；
- 测试源码使用当前 registry 与 route-planning API。

最近一次 `cargo fmt --all` 与 `cargo check --locked --all-targets` 已通过；没有运行 `cargo test`、Clippy、SDK、
真实 Provider 或负载验证。下一项工作开始前应先补做与改动风险相称的回归验证，再建立独立焦点。

## 关联文档

- [代码注册表与路由](configuration-and-routing.md)
- [当前实现说明](../implementation-status/current-implementation.md)
- [当前代码架构](../implementation-status/current-architecture.md)
