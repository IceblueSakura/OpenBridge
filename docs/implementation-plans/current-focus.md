# 当前开发焦点

## 状态

**代码注册表迁移已完成；当前无活动焦点。**

已完成边界：

- 删除 Provider/Model `routes.toml` 和 reload；
-以 `BootstrapPolicy + compiled_definition()` 构建不可变 `RegistrySnapshot`；
-将 OpenAI descriptor、字段转换、认证、响应、错误和 discovery 行为集中到独立 Provider 文件；
-以 typed definition 维护 model、deployment、alias、reasoning level 和 capability；
-迁移测试与文档，不保留旧 schema 兼容入口。

默认格式化、测试和 clippy 已通过，仓库不再把运行时配置描述为 Provider 来源。下一项工作开始前应
重新检查 live source、测试和真实 Provider 需求，再建立独立焦点。

## 关联文档

- [代码注册表与路由](configuration-and-routing.md)
- [当前实现说明](../implementation-status/current-implementation.md)
