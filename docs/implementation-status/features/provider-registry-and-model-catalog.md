# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 状态

**已完成（当前 checkout）。** Provider 和模型目录由 Rust 代码显式编译为不可变 `RuntimeRegistry`；当前没有动态 Provider DSL、自动发现
或按名称隐式聚合，Provider 池只来自显式 source 注册。

## 已完成内容

- 注册表分离 canonical Model、Provider instance、credential pool、Upstream Target、Upstream API、Route 和 Public Model 的所有权。
- 当前内置 Provider family 为 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo、ChatGPT、NVIDIA、阿里云百炼 Model Studio 和 Kimi CN；
  ChatGPT 使用独立 OAuth manager，固定 target 提供 Responses Native；Responses-only Public Model 的 Chat coverage 由编译器自动补充
  受限 Chat Bridge，已由其他 source 完整覆盖 Native 的 merged Public Model 不重复生成该自动 Bridge。
- OpenAI 现在编译 `openai-main`、`openai-gpt-5-5`、`openai-gpt-5-6-luna`、`openai-gpt-5-6-terra` 四个 generation Target，
  以及 `openai-text-embedding-3-small` Embeddings Target；它们都使用 `openai-primary` API-key pool。新增的三个 generation
  Target 只绑定 canonical profile，不新增下游 Public Model 或 Route；`openai-primary` 缺失或为空时这些 Target 保留在注册表中但配置态禁用。
- NVIDIA 与百炼分别固定到 `https://integrate.api.nvidia.com/v1` 和
  `https://dashscope.aliyuncs.com/compatible-mode/v1`，使用独立 API-key pool。NVIDIA 将 `minimax/minimax-m3` 绑定为 Chat Native
  `minimax-m3`；百炼将 `z-ai/glm-5.2` 绑定为 Chat Native + Responses Bridge，并将 `qwen/qwen3.7-plus` 与
  `qwen/qwen3.7-max` 绑定为双协议 Native Public Model。
  百炼另外将
  `qwen/qwen3.8-max`、`qwen/qwen-image-3.0`、`qwen/qwen-image-3.0-pro`、
  `qwen/qwen-audio-3.0-asr-flash`、`qwen/qwen3.5-livetranslate-flash-realtime` 与
  `qwen/qwen3.6-27b` 编译为固定 Chat Upstream Target，并将 `qwen/qwen3.7-text-embedding` 编译为固定
  Embeddings Upstream Target；这些条目暂不加入 Public Model 或 Route。
- Kimi CN 固定到 `https://api.moonshot.cn`，使用独立的 `kimi-primary` API-key pool 和 OpenAI-compatible Chat adapter；
  `moonshotai/kimi-k3` 绑定为 `kimi-k3` Public Model，提供 `/v1/chat/completions` Chat Native，并自动补充一个
  `Responses-via-Chat` Bridge Route。
- 当前可调用的 generation Public Model 为 `gpt-5.6-sol`、`LongCat-2.0`、`deepseek-v4-pro`、`deepseek-v4-flash`、`mimo-v2.5-pro`、
  `mimo-v2.5`、`mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 和 `mimo-v2.5-tts-voiceclone`，以及
  `minimax-m3`、`kimi-k3`、`glm-5.2`、`qwen3.7-plus`、`qwen3.7-max`、
  `gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna` 和 `gpt-5.6-terra`；
  `text-embedding-3-small` 是独立 Embeddings Public Model。
- `gpt-5.6-sol` 显式绑定 OpenAI 与 ChatGPT 两个 source，按 OpenAI、ChatGPT 顺序保留候选，并按可执行候选的最小公共契约公开；
  `deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个双协议 Native source，并在 Chat/Responses 内都按该顺序保留候选。
  其他当前 generation Public Model 仍按各自注册项使用一个 Provider source。
- Canonical Model ID 保持 `designer/model`，Upstream Target 同时保存并校验 `canonical_model` 与 `provider_model` 两个分层身份；
  `provider_model` 使用 `provider/model`，而下游只接触不带前缀的 Public Model 名称。
- 启动时从私有凭证配置派生的 active pool 集合只会收窄已注册 Target；缺失、无 source 或空 API-key pool 会让引用它的 Target 和
  Public Model 在本次运行中不可执行，但不会从代码注册表删除 Provider 或 Model。
- Public Model 编译先统计 Chat/Responses Native coverage，再按 source 顺序生成 Native candidates；缺失某一 downstream protocol 时，
  才从相反 Native surface 自动补充同顺序 Bridge candidates。显式双协议 Bridge surface 仍保留已声明 Bridge；注册表保存固定
  Route 顺序，不由请求重排。
- canonical Model profile 可以存在但未绑定可执行 Route；只有进入 Public Model 且通过启动校验的条目才可被客户端调用。
- MiMo 四个专用语音模型各自绑定一个 Chat Native target/API profile；它们不共享 `mimo-v2.5` 的双协议 surface，也不通过 Bridge 或
  Provider-wide audio bool 互相扩展能力。具体 ASR/TTS/VoiceDesign/VoiceClone 契约见 [Native MiMo 音频专题](native-mimo-audio.md)。

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
- [`tests/example_config.rs`](../../../tests/example_config.rs) 覆盖 OpenAI/ChatGPT target、显式 Provider 池、Public Model/Route 的固定编译事实和能力收窄。
- [`tests/native_routing_contract.rs`](../../../tests/native_routing_contract.rs) 覆盖候选顺序、Public Model 与 Route 规划。
- [`tests/provider_contract.rs`](../../../tests/provider_contract.rs) 和 [`tests/provider_boundary_contract.rs`](../../../tests/provider_boundary_contract.rs)
  覆盖 Provider 请求、认证和受信出站边界。
- [`tests/capability_definition_contract.rs`](../../../tests/capability_definition_contract.rs) 覆盖能力定义的合法性和收窄规则。
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 MiMo 专用语音模型的 Chat JSON/SSE 透传与 task-specific zero-egress 拒绝。

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
- 实现后同一聚焦测试通过（1 项），覆盖四个 trusted endpoint、Target、upstream model、credential pool、Public Model、Chat Native、
  自动 Responses Bridge Route 和本地请求规划。
- `cargo fmt -- --check`：通过；
- `cargo test --locked`：通过；
- `cargo clippy --locked -- -D warnings`：通过；
- `git diff --check`：通过。

本轮没有执行真实 NVIDIA/百炼请求、Models probe、外部 SDK、负载或长期运行测试；静态 Target 与规划测试不证明远端模型、协议、账号、
区域、网络或配额可用。

2026-08-08 百炼新增 Qwen Target 绑定的确定性验证：

- 实现前运行 `cargo test --locked --test example_config bailian_qwen_models_compile_as_fixed_chat_targets`，按预期因新增 Target 不存在失败；
- 实现后同一聚焦测试通过（1 项），覆盖 4 个 canonical/provider model identity、固定 endpoint、credential pool、quota scope、fault domain、
  Chat Upstream API 和 Bailian upstream model；
- `cargo test --locked --test example_config bailian_deepseek_models_compile_as_chat_native_fallbacks`：通过（1 项），并保留手动修改的
  `deepseek-v4-flash-0731` upstream model；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

本轮未将新增 Qwen Target 加入 Public Model/Route，也未执行真实百炼请求、图片生成 API、WebSocket 实时翻译、外部 SDK、负载或长期运行测试。

2026-08-08 Kimi CN Provider 与 Kimi K3 Chat Native 绑定的确定性验证：

- 实现前运行 `cargo test --locked --test example_config kimi_cn_k3_compiles_as_a_chat_native_route`，按预期因缺少 `ProviderKind::KimiCn` 失败；
- 实现后 `cargo test --locked --test example_config kimi_cn_k3_compiles_with_native_chat_and_auto_responses_bridge` 通过（1 项），覆盖
  Provider、API-key pool、可信 endpoint、canonical/provider/upstream model identity、Chat Native/自动 Responses Bridge Route、两协议
  本地规划和 `/v1/chat/completions` adapter model 替换；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

本轮未执行真实 Moonshot 请求、Models probe、外部 SDK、负载或长期运行测试；静态 Target、规划和 adapter 测试不证明真实账号权限、模型可用性、
网络、配额或远端协议行为。

2026-08-08 Native-first 与缺失协议 Bridge 自动补全：

- `cargo test --locked --test example_config kimi_cn_k3_compiles_with_native_chat_and_auto_responses_bridge`：通过（1 项）；
- `cargo test --locked route_compiler::tests`：通过（4 项），覆盖 Chat-only、Responses-only 自动补全、完整 Native coverage 抑制冗余
  自动 Bridge，以及既有显式双协议 Bridge 顺序；
- `cargo test --locked --test example_config -- --skip compiled_model_catalog_preserves_registered_model_facts`：通过（14 项），覆盖 Kimi、
  ChatGPT、DeepSeek、NVIDIA、百炼、MiMo 和公共 Route/能力契约；`cargo test --locked --test provider_contract`：通过（7 项）；
- `cargo test --locked --test forwarding_contract`：通过（46 项）；`cargo test --locked --test native_routing_contract`：通过（18 项）；
  `cargo test --locked --lib`：通过（60 项）；
- `cargo fmt -- --check`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。

上述完整测试记录早于本次 Qwen 模型与 Bailian Target 变更；本次按要求未重新执行 Rust 测试、真实 Provider、外部 SDK、负载或长期运行测试。

2026-08-08 Qwen 模型替换与 Bailian Target 扩展：

- 移除 `qwen/qwen-image-2.0-pro` canonical profile 及其 Bailian Chat Target；新增的 Qwen Image 3.0、Qwen Image 3.0 Pro、Qwen Audio 3.0 ASR Flash、Qwen3.8 Max、Qwen3.5 LiveTranslate Flash Realtime 与 Qwen3.6 27B 均绑定固定 Bailian Chat Target。
- `qwen/qwen3.7-text-embedding` 绑定固定 Bailian Embeddings Target，使用 `/embeddings` 兼容接口，并保留百炼公开的输入、维度、批量和 token 限制；该条目仍未加入 Public Model/Route。
- 本次只执行 `cargo fmt -- --check` 与 `git diff --check`；按要求未执行 Rust 测试或真实 Bailian 请求。

2026-08-08 OpenAI generation Target binding：

- `openai/gpt-5.5`、`openai/gpt-5.6-luna` 和 `openai/gpt-5.6-terra` 新增固定 OpenAI Target；每个 Target 使用对应的
  `openai/<model>` routing identity、OpenAI upstream model、`openai-primary` API-key pool 和 Chat/Responses Native API。
- `tests/example_config/providers.rs::openai_generation_profiles_compile_as_fixed_api_key_targets` 在实现前因 Target 不存在失败，
  实现后通过，并确认不激活 `openai-primary` 时这些 Target 保留但 disabled。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。
- 本次未修改私有 credential TOML，未运行真实 OpenAI Provider、probe、外部 SDK、负载或长期运行验收。

2026-08-08 ChatGPT 下游 Public Model 命名：

- ChatGPT GPT-5.3 Codex Spark、GPT-5.5、GPT-5.6 Luna/Terra/Sol 的下游 Public Model id 分别为
  `gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-terra` 与 `gpt-5.6-sol`；canonical model、Provider-qualified
  routing identity、固定 target/Route 和上游 model slug 未改变；旧的 `chatgpt-gpt-*` 下游名称不再编译为 Public Model。
- 实现前运行聚焦注册测试，按预期因新裸名尚未注册而失败；实现后注册测试与新增的标准/扩展 Models list/retrieve 契约均通过，
  并确认旧 Public Model 名称不存在。
- `cargo test --locked --test example_config`：通过（17 项）；`cargo test --locked --test forwarding_contract`：通过（47 项）。其中
  registry planning 契约证明新名称可选择原 Chat→Responses Bridge，Responses forwarding 契约证明新名称保持原上游 slug。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。
- 本轮未执行真实 ChatGPT Provider、Models probe、外部 SDK、负载、长期运行或浏览器验收；历史真实调用表保留当时的旧名称并已明确标注。

2026-08-08 非 GPT/NVIDIA reasoning level 固定契约：

- Reasoning level 是 Canonical Model 事实，同一模型的 Chat/Responses interface 不再定义不同集合：Qwen3.7 Max/Plus 为
  `none/minimal/low/medium/high/xhigh/max`，MiMo V2.5/Pro 为 `none/low/medium/high`，LongCat 2.0 为 `none/high`。
- Qwen3.7 与 MiMo V2.5/Pro 都只编译 Chat/Responses Native Route；Native Responses 原样传递 effort。只有 thinking 开关的
  Chat API 将 `none` 转为关闭、其余该模型已声明 level 转为开启；这不缩减 Models 元数据。
- 当前确定性测试覆盖完整 canonical 集合、两个 interface 的一致 Models 投影、全部档位规划、Qwen/MiMo Native Responses 原值以及
  Bailian/LongCat/MiMo Chat switch；Qwen Chat/Responses output 分别固定为 `PlainText`/`Summary`。MiMo ASR/TTS target 继续显式
  收窄 reasoning 为 `Unknown`。
- 真实下游 key 既有证据只覆盖这六个模型的 high JSON/SSE 和 Qwen Chat `none`；没有真实复测本轮新增的 Bailian Qwen Native
  Responses、Qwen 其余五档、MiMo `none/low/medium` 或 LongCat `none`。实现和测试不依赖 Hermes。
- 新契约实现前，`cargo test --locked --test example_config` 按预期 18 通过、4 失败，`cargo test --locked --test provider_contract`
  按预期 6 通过、3 失败；实现后分别 22/22 与 9/9 通过。
- 官方 Responses output schema 复核后，新增的 `Summary` 聚焦断言先按预期失败，修正 Bailian Responses ceiling 后通过。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。
- 本轮未运行真实 Provider、外部 SDK、Models HTTP 端点、负载或长期运行验收，也未修改私有 credential。

## 相关文档

- [功能需求：Model 目录与 Provider 接入配置](../../functional-requirements/model-catalog-configuration.md)
- [Public Model 与能力预检](models-api-and-capability-preflight.md)
- [Provider 实施与实测状态](../providers/README.md)
- [当前代码架构](../current-architecture.md)
- [Kimi CN Provider 状态](../providers/kimi-cn.md)
- [NVIDIA MiniMax M3 与百炼 GLM/Qwen 官方入口快照](../../references/providers/nvidia/nvidia-bailian-chat-models-2026-08-08.md)
