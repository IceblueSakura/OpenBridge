# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 状态

**已完成（当前 checkout）。** Provider 和模型目录由 Rust 代码显式编译为不可变 `RuntimeRegistry`；当前没有动态 Provider DSL、自动发现
或按名称隐式聚合，Provider 池只来自显式 source 注册。

## 已完成内容

- 注册表分离 canonical Model、Provider instance、credential pool、Upstream Target、Upstream API、Route 和 Public Model 的所有权。
- 当前内置 Provider family 为 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo、ChatGPT、NVIDIA 和阿里云百炼 Model Studio；
  ChatGPT 使用独立 OAuth manager，五个固定 target 各自提供一个 Responses Native Route 和一个受限 Chat Bridge Route。
- NVIDIA 与百炼分别固定到 `https://integrate.api.nvidia.com/v1` 和
  `https://dashscope.aliyuncs.com/compatible-mode/v1`，各自拥有基础 OpenAI-compatible Chat adapter 与独立 API-key pool。NVIDIA
  将 `minimax/minimax-m3` 绑定为 `minimax-m3`，百炼将 `z-ai/glm-5.2`、`qwen/qwen3.7-plus` 与
  `qwen/qwen3.7-max` 绑定为对应 Public Model；四者都只有一个 Chat Native Route。
- 当前可调用的 generation Public Model 为 `gpt-5.6-sol`、`LongCat-2.0`、`deepseek-v4-pro`、`deepseek-v4-flash`、`mimo-v2.5-pro` 和
  `mimo-v2.5`，以及 `minimax-m3`、`glm-5.2`、`qwen3.7-plus`、`qwen3.7-max`、
  `chatgpt-gpt-5.3-codex-spark`、`chatgpt-gpt-5.5`、`chatgpt-gpt-5.6-luna` 和 `chatgpt-gpt-5.6-terra`；
  `text-embedding-3-small` 是独立 Embeddings Public Model。
- `gpt-5.6-sol` 显式绑定 OpenAI 与 ChatGPT 两个 source，按 OpenAI、ChatGPT 顺序保留候选，并按可执行候选的最小公共契约公开；
  `deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个双协议 Native source，并在 Chat/Responses 内都按该顺序保留候选。
  其他当前 generation Public Model 仍按各自注册项使用一个 Provider source。
- Canonical Model ID 保持 `designer/model`，Upstream Target 同时保存并校验 `canonical_model` 与 `provider_model` 两个分层身份；
  `provider_model` 使用 `provider/model`，而下游只接触不带前缀的 Public Model 名称。
- 启动时从私有凭证配置派生的 active pool 集合只会收窄已注册 Target；缺失、无 source 或空 API-key pool 会让引用它的 Target 和
  Public Model 在本次运行中不可执行，但不会从代码注册表删除 Provider 或 Model。
- 同一 downstream operation 内先编译 Native candidates，再编译同顺序的 Bridge candidates；注册表保存固定 Route 顺序，不由请求重排。
- canonical Model profile 可以存在但未绑定可执行 Route；只有进入 Public Model 且通过启动校验的条目才可被客户端调用。

## 实现边界

- 编译入口为 [`src/providers/catalog.rs`](../../../src/providers/catalog.rs) 和
  [`src/providers/catalog/routing.rs`](../../../src/providers/catalog/routing.rs)，校验与运行实体位于
  [`src/registry/`](../../../src/registry/)。
- 服务启动可将私有凭证文件解析出的 active pool 集合传给注册表编译器；该集合只能收窄已注册 Target，不能新增 Provider、pool、Route、endpoint 或能力。
- `ProviderAdapter` 负责 Provider 侧请求、认证、响应和错误边界；pipeline 不按 Provider 名称分支，也不根据请求创建新的 Route。
- 当前注册的 Provider 主要是 OpenAI-compatible Native surface；不因此宣称真实异构协议 Provider 已完成。

## 验证证据

- [`tests/config_contract.rs`](../../../tests/config_contract.rs) 覆盖注册项、引用和启动校验。
- [`tests/upstream_credential_config.rs`](../../../tests/upstream_credential_config.rs) 覆盖 active pool 筛选、Target/Public Model 过滤和
  非激活 pool 不进入服务凭证要求。
- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖五个 ChatGPT target、显式 Provider 池、Public Model/Route 的固定编译事实和能力收窄。
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖候选顺序、Public Model 与 Route 规划。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 和 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider 请求、认证和受信出站边界。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖能力定义的合法性和收窄规则。

这些测试证明当前代码注册表和进程内规划行为，不证明 Provider 目录的外部可用性或动态配置能力。

2026-08-07 Provider 池与模型命名分层变更的确定性验证：

- `cargo fmt --all -- --check`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

本次未执行真实 Provider、外部 SDK、负载、长期运行或浏览器验收；已有 ChatGPT 真实调用记录仍只代表当时的账号、网络、backend 和
payload。

2026-08-08 NVIDIA 与百炼 Provider 基础注册的确定性验证：

- 实现前运行 `cargo test --locked --test provider_contract nvidia_and_bailian_adapters_bind_only_the_confirmed_chat_surface`，按预期因缺少
  `ProviderKind::Nvidia` 与 `ProviderKind::Bailian` 失败；
- 实现后同一聚焦测试通过（1 项）；
- 当时的 `nvidia_and_bailian_are_compiled_as_unbound_api_key_provider_profiles` 聚焦测试通过（本轮绑定模型后已重命名并移除“无 Target”断言）；
- `cargo test --locked --test example_config compiled_provider_credential_pools_are_shared_and_match_the_private_toml_example`：通过（1 项）；
- `cargo fmt -- --check`、`cargo test --locked` 与 `cargo clippy --locked -- -D warnings`：通过。

2026-08-08 NVIDIA MiniMax M3 与百炼 GLM/Qwen Chat 绑定的确定性验证：

- 实现前运行 `cargo test --locked --test example_config nvidia_and_bailian_models_compile_as_chat_native_routes`，按预期因缺少首个 NVIDIA
  Target 失败；
- 实现后同一聚焦测试通过（1 项），覆盖四个 trusted endpoint、Target、upstream model、credential pool、Public Model、唯一 Chat
  Native Route 和本地请求规划。
- `cargo fmt -- --check`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

本轮没有执行真实 NVIDIA/百炼请求、Models probe、外部 SDK、负载或长期运行测试；静态 Target 与规划测试不证明远端模型、协议、账号、
区域、网络或配额可用。

## 相关文档

- [功能需求：Model 目录与 Provider 接入配置](../../functional-requirements/model-catalog-configuration.md)
- [Public Model 与能力预检](models-api-and-capability-preflight.md)
- [当前代码架构](../current-architecture.md)
- [NVIDIA MiniMax M3 与百炼 GLM/Qwen 官方入口快照](../../references/providers/nvidia-bailian-chat-models-2026-08-08.md)
