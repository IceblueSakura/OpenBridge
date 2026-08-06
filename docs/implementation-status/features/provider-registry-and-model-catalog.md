# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 状态

**已完成（当前 checkout）。** Provider 和模型目录由 Rust 代码显式编译为不可变 `RuntimeRegistry`；当前没有动态 Provider DSL、自动发现
或按 canonical model ID 隐式聚合。

## 已完成内容

- 注册表分离 canonical Model、Provider instance、credential pool、Upstream Target、Upstream API、Route 和 Public Model 的所有权。
- 当前内置 Provider family 为 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo 和 ChatGPT；ChatGPT target 默认禁用且没有 Route/Public
  Model 数据面。
- 当前可调用的 generation Public Model 为 `gpt-5.6-sol`、`LongCat-2.0`、`deepseek-v4-pro`、`deepseek-v4-flash`、`mimo-v2.5-pro` 和
  `mimo-v2.5`；`text-embedding-3-small` 是独立 Embeddings Public Model。
- `deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个 source，按配置顺序保留候选；其他当前 generation Public Model 仍按各自
  注册项使用一个 Provider source。
- 同一 downstream operation 内先编译 Native candidates，再编译同顺序的 Bridge candidates；注册表保存固定 Route 顺序，不由请求重排。
- canonical Model profile 可以存在但未绑定可执行 Route；只有进入 Public Model 且通过启动校验的条目才可被客户端调用。

## 实现边界

- 编译入口为 [`src/providers/catalog.rs`](../../../src/providers/catalog.rs) 和
  [`src/providers/catalog/routing.rs`](../../../src/providers/catalog/routing.rs)，校验与运行实体位于
  [`src/registry/`](../../../src/registry/)。
- `ProviderAdapter` 负责 Provider 侧请求、认证、响应和错误边界；pipeline 不按 Provider 名称分支，也不根据请求创建新的 Route。
- 当前注册的 Provider 主要是 OpenAI-compatible Native surface；不因此宣称真实异构协议 Provider 已完成。

## 验证证据

- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖注册项、引用和启动校验。
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖候选顺序、Public Model 与 Route 规划。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 和 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider 请求、认证和受信出站边界。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖能力定义的合法性和收窄规则。

这些测试证明当前代码注册表和进程内规划行为，不证明 Provider 目录的外部可用性或动态配置能力。

## 相关文档

- [功能需求：Model 目录与 Provider 接入配置](../../functional-requirements/model-catalog-configuration.md)
- [Public Model 与能力预检](models-api-and-capability-preflight.md)
- [当前代码架构](../current-architecture.md)
