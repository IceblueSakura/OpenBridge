# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 状态

**已完成（当前 checkout）。** Provider 和模型目录由 Rust 代码显式编译为不可变 `RuntimeRegistry`；当前没有动态 Provider DSL、自动发现
或按名称隐式聚合，Provider 池只来自显式 source 注册。

## 已完成内容

- 注册表分离 canonical Model、Provider instance、credential pool、Upstream Target、Upstream API、Route 和 Public Model 的所有权。
- 当前内置 Provider family 为 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo、ChatGPT、NVIDIA、阿里云百炼 Model Studio 和 Kimi CN；
  ChatGPT 使用独立 OAuth manager，固定 target 提供 Responses Native，并以显式/自动受限 Chat Bridge 提供 Chat coverage；其 Upstream
  API 类型化声明 streaming required，并启用 bounded Responses SSE→JSON 转换。
- OpenAI 现在编译 `openai-main`、`openai-gpt-5-5`、`openai-gpt-5-6-luna`、`openai-gpt-5-6-terra` 四个 generation Target，
  以及 `openai-text-embedding-3-small` Embeddings Target；它们都使用 `openai-primary` API-key pool。新增的三个 generation
  Target 只绑定 canonical profile，不新增下游 Public Model 或 Route；`openai-primary` 缺失或为空时这些 Target 保留在注册表中但配置态禁用。
- OpenRouter、NVIDIA 与百炼分别固定到 `https://openrouter.ai/api/v1`、`https://integrate.api.nvidia.com/v1` 和
  `https://dashscope.aliyuncs.com/compatible-mode/v1`，使用独立 API-key pool。OpenRouter 将 `minimax/minimax-m3` 绑定为
  Chat/Responses Native，NVIDIA 保留同一 Public Model 的 Chat Native 后备；百炼将 `z-ai/glm-5.2` 绑定为 Chat Native +
  Responses Bridge，并将 `qwen/qwen3.7-plus`、`qwen/qwen3.7-max` 与 `qwen/qwen3.8-max` 绑定为双协议 Native Public Model。
  百炼另外将
  `qwen/qwen-image-3.0`、`qwen/qwen-image-3.0-pro`、
  `qwen/qwen-audio-3.0-asr-flash`、`qwen/qwen3.5-livetranslate-flash-realtime` 与
  `qwen/qwen3.6-27b` 编译为固定 Chat Upstream Target，并将 `qwen/qwen3.7-text-embedding` 编译为固定
  Embeddings Upstream Target；其中 `qwen3.7-text-embedding` 已加入唯一
  `qwen3-7-text-embedding-bailian-embeddings` Native Route 和同名 Public Model，其余 Qwen 专用条目仍暂不加入 Public Model 或 Route。
- Kimi CN 固定到 `https://api.moonshot.cn`，使用独立的 `kimi-primary` API-key pool 和 OpenAI-compatible Chat adapter；
  `moonshotai/kimi-k3` 绑定为 `kimi-k3` Public Model，提供 `/v1/chat/completions` Chat Native，并自动补充一个
  `Responses-via-Chat` Bridge Route。
- 当前可调用的 generation Public Model 为 `gpt-5.6-sol`、`LongCat-2.0`、`deepseek-v4-pro`、`deepseek-v4-flash`、`mimo-v2.5-pro`、
  `mimo-v2.5`、`mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign` 和 `mimo-v2.5-tts-voiceclone`，以及
  `minimax-m3`、`kimi-k3`、`glm-5.2`、`qwen3.7-plus`、`qwen3.7-max`、`qwen3.8-max`、
  `gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna` 和 `gpt-5.6-terra`；
  `text-embedding-3-small` 与 `qwen3.7-text-embedding` 是独立 Embeddings Public Model。
- `gpt-5.6-sol` 显式绑定 ChatGPT 与 OpenAI 两个 source，并使用 `SourceFirst` 让两个下游协议都优先 ChatGPT、再回落 OpenAI；
  固定接口仍按全部可执行候选的最小公共契约公开；
  `deepseek-v4-flash` 显式绑定 DeepSeek 与 OpenRouter 两个双协议 Native source，并在 Chat/Responses 内都按该顺序保留候选。
  `minimax-m3` 显式绑定 OpenRouter 双协议 Native 与 NVIDIA Chat Native 两个 source：Chat 按 OpenRouter、NVIDIA 排序，Responses
  只使用 OpenRouter Native。其他当前 generation Public Model 仍按各自注册项使用一个 Provider source。
- Canonical Model ID 保持 `designer/model`，Upstream Target 同时保存并校验 `canonical_model` 与 `provider_model` 两个分层身份；
  `provider_model` 使用 `provider/model`，而下游只接触不带前缀的 Public Model 名称。
- 启动时从私有凭证配置派生的 active pool 集合只会收窄已注册 Target；缺失、无 source 或空 API-key pool 会让引用它的 Target 和
  Public Model 在本次运行中不可执行，但不会从代码注册表删除 Provider 或 Model。
- 每个 generation Public Model 显式声明 `NativeFirst` 或 `SourceFirst`。前者按协议排列全部 Native 后再排列 Bridge；后者按协议先
  保持 source 顺序、再在 source 内优先 Native。缺失某一 downstream protocol 时才自动补充 Bridge；显式 Bridge surface 可在其他
  source 已有 Native coverage 时保留。注册表保存固定 Route 顺序，请求能力不会筛选或重排候选。
- `UpstreamStreamingPolicy` 显式区分 optional streaming 与 required streaming；required API 的非流式转换开关只能关闭，或选择
  Responses SSE buffering。错误 operation/capability 组合在启动时失败，关闭转换的候选会把固定接口 `non_streaming` 收窄为
  `unsupported`，不会因后续候选更强而被跳过。
- generation canonical parameter 必须来自 Chat/Responses 代码内类型化目录。`UpstreamApiModelRules.disabled_parameters` 收窄当前
  API 的有效参数集合；闭合 `ignored_parameters` 只允许五类普通提示，表示 OpenBridge 下游接受但当前 API egress 删除。规则按具体 API
  和 candidate 生效，不能用于 Embeddings、输出语义字段、能力字段、输出预算或任意字符串过滤。当前 ignore 配置只保留有文档与真实
  E2E 证据的 Kimi sampling/penalty 提示和 ChatGPT seed；Kimi/MiMo/ChatGPT 的不兼容输出语义字段改为显式 disabled。
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

2026-08-08 MiniMax M3 reasoning 与 OpenRouter-first source：

- MiniMax 官方只声明 M3 thinking 可开/关，OpenRouter M3 元数据没有发布 `supported_efforts`；canonical model 因此固定为
  `none/high`，不外推 `low/medium`。
- 新增 `openrouter-minimax-m3` Chat/Responses Native Target，固定 `openrouter-primary` 与 upstream model
  `minimax/minimax-m3`；`minimax-m3` 的 Chat Route 顺序为 OpenRouter、NVIDIA，Responses 只保留 OpenRouter Native。
- 实现前，canonical 聚焦测试按预期以 `[] != [High, None]` 失败，Provider 聚焦测试按预期因缺少
  `openrouter-minimax-m3` 失败；实现后两项与 OpenRouter Chat/Responses 标准 reasoning wire 测试均通过。
- `cargo test --locked --test example_config`：23 项通过；`cargo test --locked --test provider_contract`：9 项通过。
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`：通过。
- 本轮确定性测试不证明 OpenRouter 或 NVIDIA 真实 endpoint 接受 `none/high`、返回可读 reasoning，或在 fallback 时保持相同
  Provider 行为；真实 Provider、Models HTTP probe、外部 SDK、负载和长期运行均未执行。

2026-08-09 Qwen3.7 Text Embedding Public Model/Route：

- 将已有 `bailian-qwen3-7-text-embedding` Embeddings Target 接入下游 `qwen3.7-text-embedding` Public Model，新增唯一
  `qwen3-7-text-embedding-bailian-embeddings` Native Route；标准 `/v1/models` 与扩展 `/openbridge/v1/models` 均公开该模型，
  请求规划固定使用百炼 `/embeddings` Native API。
- `cargo test --locked --test embedding_registry_contract`：通过（4 项）；当前完整 Rust baseline 也已覆盖该 Public Model、Models
  endpoint 与固定百炼 Route。
- 未执行真实百炼请求、Models probe、外部 SDK、负载或长期运行验收。

2026-08-09 类型化 Route strategy 与 streaming-only Responses 非流式转换：

- generation registration 新增显式 `NativeFirst`/`SourceFirst`；GPT 使用 `SourceFirst`。`gpt-5.6-sol` source 顺序调整为
  ChatGPT、OpenAI，因此 ChatGPT Chat Bridge 与 Responses Native 分别排在 OpenAI 候选前；普通模型的既有 Native-first 顺序不变。
- `UpstreamApiConfig` 新增 `UpstreamStreamingPolicy` 与 `NonStreamingConversion`。ChatGPT Responses 固定为 required streaming +
  `BufferResponsesSse`；其他当前 API 为 optional。错误 operation/capability 组合在 registry startup 校验失败。
- 扩展 Models generation interface 新增 `non_streaming`。该值按全部候选保守相交；首选候选关闭转换时，非流式请求会在 egress 前由
  capability preflight 拒绝，不会跳过到后续 optional candidate。
- 下游非流式 ChatGPT Chat/Responses 会强制上游 `stream: true`，在 JSON/SSE limit 内完整校验 Responses lifecycle；转换器用
  response snapshots、有序 `response.output_item.done` 与显式 terminal 组装 Native response，Chat 再使用既有非流式 Bridge。非法
  SSE、非 SSE success、超限 body 与缺少 terminal 均返回安全 502。
- 稀疏 terminal 由已验证的 `output_item.done` 补齐；普通 Provider 的 stream success 必须显式携带 SSE media type，ChatGPT 的静态
  profile 允许真实 backend 缺失该 header 后规范化下游响应。完成 output 中的 opaque reasoning continuation 不进入无状态 Chat response。
- route compiler、stream takeover、Bridge、`example_config` 与 `forwarding_contract` 的确定性测试通过；五个 GPT 的真实 40 单元
  Chat/Responses × stream on/off × omitted/high 最终全部通过。实现与测试不依赖 Hermes/Codex runtime。
- 未执行真实 OpenAI API-key fallback、外部 SDK、负载或长期运行验收。

2026-08-09 Bailian Qwen3.8 Max Public Model 与 Qwen3.6 27B canonical facts：

- `qwen3.8-max` 现在通过 `bailian-qwen3-8-max` 提供 Chat/Responses 双协议 Native Route，并进入标准/扩展 Models HTTP 投影；
  Chat reasoning output 为 `PlainText`，Responses 为 `Summary`，两接口统一公开 `none/minimal/low/medium/high/xhigh/max`；
- Bailian Chat adapter 将 Qwen3.8 的 `none` 转为 `enable_thinking=false`、其他六档转为 `true`，Native Responses 保留原 effort；
  未公开图片、视频、工具或结构化输出能力；
- `qwen/qwen3.6-27b` 的 model-level 参数集合保持不变，input 上限修正为与 262,144 context 相同，Alibaba endpoint 输出上限保持
  65,536；reasoning 只有开关证据，因此固定为 `none/high`，本轮仍不创建 Public Model/Route；
- 新增聚焦测试在实现前按预期失败，随后 `cargo test --locked --test example_config qwen3`、两个 Provider reasoning 聚焦测试和
  Models HTTP 聚焦测试通过；`cargo test --locked` 首次仅有既有 OTLP trace 数量时序断言失败，单项重跑通过，第二次完整命令通过；
  `cargo clippy --locked -- -D warnings` 通过；
- 真实 `openbridge-probe` 的 Models/Chat/Responses 三项均为 HTTP 200 且远端列表包含 `qwen3.8-max`。真实下游 Qwen3.8
  Chat/Responses × JSON/SSE × none/high 为 8/8，通过后追加的 Responses 其余五档为 5/5；直接 Qwen3.6 Chat thinking off/on 为
  2/2，分别无/有 reasoning；
- 本次没有运行完整全模型端到端矩阵、外部 SDK、负载或长期运行验收，也没有修改私有 credential。

## 相关文档

- [功能需求：Model 目录与 Provider 接入配置](../../functional-requirements/model-catalog-configuration.md)
- [Public Model 与能力预检](models-api-and-capability-preflight.md)
- [Provider 实施与实测状态](../providers/README.md)
- [当前代码架构](../current-architecture.md)
- [Kimi CN Provider 状态](../providers/kimi-cn.md)
- [OpenRouter 模型目录与 reasoning 复核](../../references/providers/openrouter/models.md)
- [NVIDIA MiniMax M3 与百炼 GLM/Qwen 官方入口快照](../../references/providers/nvidia/nvidia-bailian-chat-models-2026-08-08.md)
