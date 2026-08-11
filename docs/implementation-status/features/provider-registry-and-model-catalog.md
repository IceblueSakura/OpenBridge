# 功能：Provider、Model、Target、API、Route 与 Public Model 注册表

## 状态

**已完成（当前 checkout）。** Provider、33 个 canonical Model leaf、36 个固定 Upstream Target 和 25 个 Public Model 由 Rust
代码显式编译为不可变 `RuntimeRegistry`；每个 Model 必须选择一个闭合 canonical task variant。当前没有动态 Provider DSL、自动发现
或按名称隐式聚合，Provider 池只来自显式 source 注册。

## 已完成内容

- 注册表分离 canonical Model、Provider instance、credential pool、Upstream Target、Upstream API、Route 和 Public Model 的所有权。
- 33 个 canonical Model leaf 全部使用必填 `CanonicalModelTask`：25 个 `Generation`、3 个 `Embedding`、2 个
  `SpeechRecognition`，以及各 1 个 `SpeechSynthesis`、`VoiceDesign`、`VoiceClone`。`ModelConfig` 与运行期 `ModelInfo`
  都只在公共 identity envelope 外保存这一 task union；context、modalities、普通参数和 reasoning 位于对应 payload，运行实体不保留
  `ModelMode`、flat shadow fields 或第二套可漂移 task 状态。
- Generation reasoning 只由 `ReasoningProfile::Unsupported | Unknown | Supported { levels }` 保存；`levels` 是有序、唯一的 checked set。
  普通参数不再保存 `reasoning`/`reasoning_effort` sentinel，Public Model compiler 按 `ReasoningProfile + downstream protocol`
  派生相应 wire parameter。非 Generation task 的 reasoning-unsupported 与固定 modalities 由 task variant 派生。
- Registry 在构造 snapshot 前先执行 Provider capability ceiling containment，再执行闭合 canonical task/executable profile matrix；
  Embeddings 只接受 `Embedding`，Responses 只接受 `Generation`，Chat 专用语音 task 只接受同名 audio profile，Generation 的
  `AudioUnderstanding` 还要求 canonical input 明确包含 Audio 且 output 明确包含 Text。越界分别返回 `CapabilityElevation` 或
  `UpstreamApiModelTaskMismatch`，不会 late-filter 或 panic。
- 当前内置 Provider family 为 OpenAI、LongCat、OpenRouter、DeepSeek、Xiaomi MiMo、ChatGPT、NVIDIA、阿里云百炼 Model Studio 和 Kimi CN；
  ChatGPT 使用独立 OAuth manager，固定 target 提供 Responses Native，并以显式/自动受限 Chat Bridge 提供 Chat coverage；其 Upstream
  API 类型化声明 streaming required，并启用 bounded Responses SSE→JSON 转换。
- OpenAI 现在编译 `openai-main`、`openai-gpt-5-5`、`openai-gpt-5-6-luna`、`openai-gpt-5-6-terra` 四个 generation Target，
  以及 `openai-text-embedding-3-small` Embeddings Target；它们都使用 `openai-primary` API-key pool。新增的三个 generation
  Target 只绑定 canonical profile，不新增下游 Public Model 或 Route；`openai-primary` 缺失或为空时这些 Target 保留在注册表中但配置态禁用。
- OpenRouter、NVIDIA 与百炼分别固定到 `https://openrouter.ai/api/v1`、`https://integrate.api.nvidia.com/v1` 和
  `https://dashscope.aliyuncs.com/compatible-mode/v1`，使用独立 API-key pool。OpenRouter 将 `minimax/minimax-m3` 绑定为
  Chat/Responses Native，NVIDIA 保留同一 Public Model 的 Chat Native 后备；OpenRouter 另将 `google/gemma-4-31b-it` 绑定为
  双协议 Native `gemma-4-31b-it` Public Model。NVIDIA 将 `nvidia/nemotron-3-embed-1b` 绑定为独立 Embeddings Public Model；
  百炼将 `z-ai/glm-5.2` 绑定为 Chat Native +
  Responses Bridge，并将 `qwen/qwen3.7-plus`、`qwen/qwen3.7-max` 与 `qwen/qwen3.8-max` 绑定为双协议 Native Public Model。
  百炼另外将 `qwen/qwen-image-3.0`、`qwen/qwen-image-3.0-pro`、
  `qwen/qwen3.5-livetranslate-flash-realtime` 与 `qwen/qwen3.6-27b` 编译为固定 Chat Upstream Target，并将
  `qwen/qwen3.7-text-embedding` 编译为固定 Embeddings Upstream Target。`qwen3.6-27b` 已加入 Chat Native、Responses-via-Chat
  Bridge Route 和同名 Public Model；`qwen3.7-text-embedding` 已加入唯一
  `qwen3-7-text-embedding-bailian-embeddings` Native Route 和同名 Public Model。Qwen Image 与 LiveTranslate 条目仍暂不加入
  Public Model 或 Route。
  `qwen/qwen-audio-3.0-asr-flash` 保留为 `SpeechRecognition` canonical Model；因当前没有已确认的 Bailian Chat executable ASR
  profile，其原 generic Chat Target 已删除，也没有 Route/Public Model，不能仅凭 model-list 可见性获得可调用能力。
- Kimi CN 固定到 `https://api.moonshot.cn`，使用独立的 `kimi-primary` API-key pool 和 OpenAI-compatible Chat adapter；
  `moonshotai/kimi-k3` 绑定为 `kimi-k3` Public Model，提供 `/v1/chat/completions` Chat Native，并自动补充一个
  `Responses-via-Chat` Bridge Route。
- 当前可调用的 Generation Public Model 为 `gpt-5.6-sol`、`LongCat-2.0`、`deepseek-v4-pro`、`deepseek-v4-flash`、
  `mimo-v2.5-pro`、`mimo-v2.5`、`minimax-m3`、`kimi-k3`、`glm-5.2`、`qwen3.7-plus`、`qwen3.7-max`、`qwen3.8-max`、`qwen3.6-27b`、
  `gpt-5.3-codex-spark`、`gpt-5.5`、`gpt-5.6-luna` 和 `gpt-5.6-terra`；
  `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign`、`mimo-v2.5-tts-voiceclone` 分别是
  `SpeechRecognition`、`SpeechSynthesis`、`VoiceDesign`、`VoiceClone` Public Model；
  `text-embedding-3-small`、`qwen3.7-text-embedding` 与 `nemotron-3-embed-1b` 是独立 Embeddings Public Model；
  `gemma-4-31b-it` 是 OpenRouter 双协议 Native Generation Public Model。
- `gpt-5.6-sol` 显式绑定 ChatGPT 与 OpenAI 两个 source，并使用 `SourceFirst` 让两个下游协议都优先 ChatGPT、再回落 OpenAI；
  固定接口仍按全部可执行候选的最小公共契约公开；
- `deepseek-v4-pro` 显式绑定 DeepSeek 与 Bailian 两个 Chat Native source，并在 Responses 缺少 Native coverage 时按相同顺序补充 Bridge；
- `deepseek-v4-flash` 显式绑定 DeepSeek、Bailian 与 OpenRouter 三个 source，并使用 `SourceFirst`。Chat 按 DeepSeek、Bailian、
  OpenRouter 排列三个 Native 候选；Responses 因 Bailian 只有 Chat Native surface 而保持 DeepSeek、OpenRouter 两个 Native 候选；
- `minimax-m3` 显式绑定 OpenRouter 双协议 Native 与 NVIDIA Chat Native 两个 source：Chat 按 OpenRouter、NVIDIA 排序，Responses
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
- canonical Model profile 可以存在但未绑定可执行 Route；只有进入 Public Model 且通过启动校验的条目才可被客户端调用。Public Models
  的 `capabilities.tasks` 从唯一 canonical task 映射，不再从 Route operation 硬编码；跨 operation 混合 canonical task、或同 task
  同 audio variant 的 payload 交集为空，都会以 typed registry error 拒绝整个 snapshot。
- MiMo 四个专用语音模型各自绑定一个 Chat Native target/API profile；它们不共享 `mimo-v2.5` 的双协议 surface，也不通过 Bridge 或
  Provider-wide audio bool 互相扩展能力。具体 ASR/TTS/VoiceDesign/VoiceClone 契约见 [Native MiMo 音频专题](native-mimo-audio.md)。
- Provider/Target 图片配置使用 checked `ImageInputCapabilities` envelope 与 source-payload 判别联合，不再以 source slice、MIME、detail
  和独立 limit 组合支持状态。MiMo Chat/Responses Provider ceiling 固定为 Both：64 parts、8,192-byte Remote URL、
  JPEG/PNG/GIF/WebP/BMP data URL，inline encoded/decoded 单项与累计预算均为 50 MiB/38 MiB，detail 只允许省略且 default 未知；
  `mimo-v2.5` 的两个 executable API 保留该 profile，MiMo Pro 与四个专用 audio Target 都为 `None`。OpenAI Chat/Responses ceiling
  也固定为 Both：500 parts、8,192-byte Remote URL、JPEG/PNG/GIF/WebP data URL，inline 单项为 20 MiB/15 MiB、累计为
  50 MiB/38 MiB，detail default 为 `auto` 且显式接受 `auto/low/high/original`；所有 checked-in OpenAI executable Target 仍为
  `None`。Provider ceiling 不自动打开 Target。
- 静态图片 source union 只有 Remote URL、data URL 与 Both；旧 OpenAI ceiling 的 `FileId` 已删除。request analyzer 仍将 Responses
  `file_id` 识别为闭合 wire fact，但因为没有 resource identity、ownership、affinity 与 limit payload，Public Models 不投影它，
  preflight 在 egress 前拒绝。
- Public Model compiler 将 executable image profile 复制为私有 owned source/detail union，并在所有固定 Route candidate 上保守相交。
  Remote limit 取最小值；data MIME 取交集，四个 inline limit 取最小值并按交集后的 per-item/part 数 clamp 累计上限；data payload
  消失时可从 Both 降为 Remote-only，所有 source 消失才关闭 image。既有 Models JSON 只从该 union 单向投影，不适用 limit 的 `0`
  是 wire-only 表示；preflight 读取同一 owned contract 的 source-specific `Option` limit，不读取 flat DTO sentinel。

### 当前实际接入清单与 2026-08-09 文字复测

“实际接入”在本页指已经进入 Public Model、至少拥有一条固定 Route，且能由启动期 registry 编译为下游接口；它不包括只有 canonical
Model 或 Upstream Target 的条目。静态目录共有 25 个 Public Model，其中 18 个是同时提供 Chat/Responses 的文本 Generation 模型，
4 个是 Chat-only MiMo 专用音频模型，3 个是 Embeddings 模型。运行时还会依据 active credential pool 收窄该集合。

2026-08-09 在 `main@ed515cc` 启动当前服务后，`GET /openbridge/v1/models` 返回 21 个可执行模型：16 个文本模型全部进入
`reasoning=none/high × Chat/Responses × stream=false/true` 八项矩阵，5 个非通用文字模型单列；静态
`text-embedding-3-small` 当次没有进入运行时 Models。下表只描述正常首选路径，没有强制后备 source。

| Public Model | 固定 Provider source | Chat/Responses 路径 | 当次八项矩阵 |
|---|---|---|---|
| `gpt-5.6-sol` | ChatGPT；OpenAI 后备 | ChatGPT Responses Native；Chat Bridge；OpenAI 双协议后备 | 8/8 HTTP 200、文字和终态完整 |
| `gpt-5.3-codex-spark` | ChatGPT | Responses Native；Chat Bridge | `high` 4/4 HTTP 200；`none` 4/4 HTTP 400 `unsupported_value` |
| `gpt-5.5` | ChatGPT | Responses Native；Chat Bridge | 8/8 HTTP 200、文字和终态完整 |
| `gpt-5.6-luna` | ChatGPT | Responses Native；Chat Bridge | 8/8 HTTP 200、文字和终态完整 |
| `gpt-5.6-terra` | ChatGPT | Responses Native；Chat Bridge | 8/8 HTTP 200、文字和终态完整 |
| `LongCat-2.0` | LongCat | Chat/Responses Native，保留双向 Bridge | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `deepseek-v4-pro` | DeepSeek；Bailian 后备 | Chat Native；Responses Bridge | 8/8 HTTP 200；测试时未公开的 `none` 四项实际关闭 reasoning |
| `deepseek-v4-flash` | DeepSeek；OpenRouter；Bailian Chat 后备 | DeepSeek/OpenRouter 双协议 Native；Bailian Chat 后备 | 8/8 HTTP 200；测试时未公开的 `none` 四项实际关闭 reasoning |
| `minimax-m3` | OpenRouter；NVIDIA Chat 后备 | OpenRouter 双协议 Native；NVIDIA Chat 后备 | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `kimi-k3` | Kimi CN | Chat Native；Responses Bridge | 8/8 HTTP 200；测试时未公开的 `none` 四项实际关闭 reasoning |
| `glm-5.2` | Bailian | Chat Native；Responses Bridge | 8/8 HTTP 200；测试时未公开的 `none` 四项实际关闭 reasoning |
| `qwen3.7-plus` | Bailian | Chat/Responses Native | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `qwen3.7-max` | Bailian | Chat/Responses Native | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `qwen3.8-max` | Bailian | Chat/Responses Native | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `mimo-v2.5-pro` | MiMo | Chat/Responses Native | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |
| `mimo-v2.5` | MiMo | Chat/Responses Native | 8/8 HTTP 200；`none` 无 reasoning、`high` 有 reasoning |

128 个基础单元中，124 个为 HTTP 200，且全部有非空文字和完整 JSON/SSE 终态；另外 4 个是 Spark `none` 的最终 HTTP 400，错误为
`unsupported_value`、参数 `reasoning.effort`。64 个 `high` 单元全部成功；简单 prompt 中 15 个 GPT 单元没有可观察 reasoning，改用多步
求解 prompt 定向复测后 13 个出现 reasoning，只剩 Spark Chat JSON/SSE 仍无可观察 summary。完整逐单元矩阵和判据见
[最新真实 E2E 结果](../real-e2e-test-2026-08-08.md)。

依据上述矩阵，当前四个 canonical profile 已补入 `none`，DeepSeek V4 Pro/Flash、Kimi K3 与 GLM-5.2 的 Chat/Responses Models
投影现与已确认的关闭能力一致；Bailian DeepSeek Chat egress 只把 `none` 转为 `enable_thinking:false`。preflight 仍把显式
`ReasoningLevel::None` 当作“不要求 reasoning 能力”放行，因此没有公开 `none` 的 Spark 仍会到达当前 source 并返回 400；该边界不在
本次补全范围内。

以下条目不进入上述文字矩阵：

- 当次 Models 可见的 `mimo-v2.5-asr`、`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign`、
  `mimo-v2.5-tts-voiceclone` 是 Chat-only 专用音频 Public Model；`qwen3.7-text-embedding` 是 Embeddings-only Public Model；
- 静态 `text-embedding-3-small` 是 Embeddings-only Public Model，当次没有进入运行时 Models；
- `qwen3.6-27b` 在该 2026-08-09 快照后才加入 Public Model，因此不进入当次 16 模型文字矩阵；其 2026-08-10 接入后 8 单元补测见
  [最新真实 E2E 结果](../real-e2e-test-2026-08-08.md)；
- `openai/gpt-5.5`、`openai/gpt-5.6-luna`、`openai/gpt-5.6-terra` 以及 `qwen/qwen-image-3.0`、
  `qwen/qwen-image-3.0-pro`、`qwen/qwen3.5-livetranslate-flash-realtime` 在当次快照中只有固定 Target，没有 Public Model/Route；
- `qwen/qwen-audio-3.0-asr-flash` 只有 canonical Model，没有 executable Target、Route 或 Public Model。

其中 Qwen Image 3.0/Pro 和 LiveTranslate 的 generic Bailian Chat Target 只表示静态 trusted deployment 绑定；当前没有对应下游
image-generation/realtime interface、Route 或真实协议验收，不能把 Models 可见性或 registry 编译成功表述为可执行能力。

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
- [`tests/forwarding_contract.rs`](../../../tests/forwarding_contract.rs) 覆盖 MiMo 图片 Chat/Responses 的 data/remote、JSON/SSE 原样透传与完整
  terminal，以及专用语音模型的 Chat JSON/SSE 透传与 task-specific zero-egress 拒绝。

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

2026-08-08 Qwen 模型替换与 Bailian Target 扩展（历史记录，当前状态以上方“已完成内容”为准）：

- 当时移除 `qwen/qwen-image-2.0-pro` canonical profile 及其 Bailian Chat Target；新增的 Qwen Image 3.0、Qwen Image 3.0 Pro、
  Qwen Audio 3.0 ASR Flash、Qwen3.8 Max、Qwen3.5 LiveTranslate Flash Realtime 与 Qwen3.6 27B 当时均绑定固定 Bailian Chat Target。
  其中 Qwen Audio 3.0 ASR Flash 的 generic Chat Target 已在 2026-08-09 canonical task/audio profile 迁移中删除，canonical Model 保留。
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

2026-08-09 Provider operation presence 与单一 endpoint/capability descriptor：

- `ApiCapabilities` 以 Chat Completions、Responses、Embeddings 三个可选 typed profile 表达 Provider operation surface；
  capability payload 不再重复携带 `enabled`。Embeddings profile 删除 disabled/零值 `Default`，一旦注册就完整校验。
- 每个 OpenAI-compatible Provider 的唯一 `API_SURFACE` 同点声明 operation 的固定相对 endpoint path 与 concrete capability；
  `ProviderContract` 和 adapter 从该 descriptor 派生，`ProviderDefinition` 再从 adapter 派生唯一 Provider kind。配置不能再表达
  “contract 支持但 adapter 无 path”或相反的 fail-late 状态。
- `UpstreamApiCapabilities` variant 一旦存在就代表可执行 operation。Provider ceiling 缺席而 Target 注册该 variant 时仍在 startup
  返回 `CapabilityElevation`；Route 引用 Target 中缺席的 operation 返回结构化 `UnknownReference`；API 与 Route 同时缺席时
  Public Model 不投影该 interface，request preflight 返回 `UnsupportedProtocol`。
- Target 整体 `enabled` 保留且继续决定静态候选资格。DeepSeek model surface 与 MiMo text/image/audio target profile 改为闭合枚举；
  MiMo audio 不再先创建 Responses 后按位置截断，现有 Route 顺序、state ownership、reasoning、Public Models 与 wire path 保持不变。
- 失败优先测试先在旧结构上因 ChatGPT contract 不支持 `None`/`Some` presence 断言而产生预期编译错误；实现后 Provider、Registry、
  capability、routing、Embeddings 与 example-config 聚焦测试通过。`cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 全部通过。
- 本轮未执行真实 Provider、外部 SDK、Models HTTP probe、负载或长期运行验收，也未读取或修改私有 credential。

2026-08-09 Canonical Model task 与 executable audio profile 判别联合：

- 31 个 canonical leaf 已原子迁移到必填 task union，表驱动测试逐 ID 固定 24/2/2/1/1/1 映射；`ModelInfo` 不再复制 context、
  modalities、parameters、reasoning 或 task shadow state。
- MiMo ASR/TTS/VoiceDesign/VoiceClone 使用同名 canonical task 和 executable profile；标准/扩展 Models 分别投影
  `speech_recognition`、`speech_synthesis`、`voice_design`、`voice_clone`，普通 Generation 与 Embedding 的既有 task 投影保持不变。
- Registry 聚焦测试覆盖专用 task 缺 profile、ASR 误绑 TTS、Provider audio ceiling 越界、Generation AudioUnderstanding 的
  confirmed/unknown modality matrix、Embedding operation mismatch、跨 operation Public Model task 混合，以及同 ASR variant 的空
  language 交集；错误分别固定为 `UpstreamApiModelTaskMismatch`、`CapabilityElevation`、`PublicModelTaskMismatch` 和
  `PublicModelInterfaceProfileMismatch`。
- `qwen/qwen-audio-3.0-asr-flash` 仍在 31-leaf catalog 中并固定为 `SpeechRecognition`，但 Bailian generic Chat Target 已删除；
  `tests/example_config/providers.rs::unverified_bailian_qwen_audio_remains_canonical_without_an_executable_target` 保持该 fail-closed 事实。
- 本轮实际运行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check`，全部通过；
  `tests/example_config.rs` 23 项、`tests/embedding_definition_contract.rs` 6 项和 `tests/config_contract.rs` 20 项聚焦测试也全部通过。
- 本轮未执行真实 Provider、外部 SDK、Models HTTP probe、负载、长期运行或媒体内容验收，也未读取或修改私有 credential。

2026-08-09 Responses continuation 与 credential affinity 静态分型：

- Provider Responses ceiling 与单 Target executable state 现在使用 `ResponsesProfile<S>` 的不同静态实例；Provider ceiling 以闭合
  `Stateless | Storage | Continuation | StorageAndContinuation` 表达两个独立上界，executable 以
  `ExecutableResponsesState { storage, affinity }` 表达 Target state ownership。
- `ResponsesAffinity` 只有 `Unbound | TargetBound | TargetBoundContinuation`；只有最后一个 variant 派生
  `previous_response_id`、continuation issuer 与 single-member credential 约束。`UpstreamApiConfig`/runtime 不再另存全局
  `StateAffinity`，Responses capability 也不再另存 `store`/`previous_response_id` bool。
- OpenAI Provider ceiling 保留 storage/continuation 上界，但所有 checked-in executable Target 仍显式关闭二者；OpenAI、Bailian、
  ChatGPT、LongCat、MiMo 保持 `TargetBound`，DeepSeek、OpenRouter 保持 `Unbound`。ceiling 不会自动扩大 Target。
- Route contribution 与 Public execution interface 分别使用携带 issuer 的 private continuation union；Bridge 固定 unsupported，Public
  Models 只投影 SupportState/parameter。credential gate 扫描全部启用 Target，不依赖 Public Model；普通 Target-bound pool 仍可有多个
  member。只有请求实际携带 continuation 时关闭跨 Target fallback，无状态请求保持既有 fallback。
- TDD 首测先因新 union 类型不存在产生预期 E0432；实现后 capability、config、credential、Bridge、Provider matrix、issuer 与
  forwarding/resilience 聚焦测试通过。最终 `cargo check --locked --all-targets`、`cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check` 全部通过。
- 本轮未执行真实 Provider、外部 SDK、Models HTTP probe、负载、长期运行或多账号 continuation 验收，也未读取或修改私有 credential。

2026-08-09 图片 source-payload 判别联合与 Public Model owned intersection：

- 实现前新增的 source-payload capability contract 在旧结构上按预期编译失败，因为
  `ImageSourceCapabilities`、source-specific checked limits 与 detail policy 尚不存在；实现随后迁移 core、Provider/Target、registry owned
  contract、preflight、Models projection 与测试，不保留 flat 配置 shim。
- core/runtime 负例固定拒绝 Remote URL limit 8 与 inline encoded limit 3；最小合法 `https://a` 和
  `data:image/png;base64,AA==` 通过 compiled preflight。Provider boundary 逐项固定 MiMo/OpenAI 两个 operation ceiling 的四个 inline
  limit；Public Model 测试覆盖 remote-only/data-only/Both、MIME/detail 交集、cross-minima clamp 与 source-specific preflight；MiMo
  production registry/router 测试对 Chat/Responses 的 data JSON、remote SSE、完整 prepared upstream body 和 SSE terminal 做确定性验证。
- 本轮最终本地运行 `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与
  `git diff --check`，全部通过。
- 本轮未执行真实 MiMo/OpenAI Provider、外部 SDK、远程图片抓取或图片内容语义、负载、长期运行验收，也未读取或修改私有 credential。

2026-08-09 Structured Output 闭合 profile 与 Public 原子交集：

- `StructuredOutputProfile` 现为 `JsonObject | JsonSchema(JsonSchemaSupport) |
  JsonObjectAndJsonSchema(JsonSchemaSupport)`；`JsonSchemaSupport` 只允许 `NonStrictOnly | StrictSupported`。外层 `Option` 唯一表示
  unsupported，不再允许空/重复 mode、无 JSON Schema 却 strict、公开 slice/strict bool 或零值 `Default`。
- Provider ceiling 与 executable Target 共用完整 operation profile，并以闭合 subset 验证；Bailian、ChatGPT、DeepSeek、MiMo、OpenAI、
  OpenRouter 的既有 literal 原子迁移，没有扩大任何 Target。表驱动测试固定全部 9 个 Provider family 与 45 个 generation Target
  operation 的 profile。
- Route contribution 与 Public execution interface 直接保存 core profile；空交集成为 `None` 并同步删除 Structured Output 参数。
  Models 的 `support/modes/strict_schema` 只由 serializer 临时投影，request analyzer/preflight 使用独立 request union 与同一 compiled
  capability 双 enum match，不读取 DTO。
- 首个 TDD 用例在旧实现上按预期暴露 `supported + []`；实现后 core truth table、Provider elevation、Public strict 降级、请求分析、
  MiMo/DeepSeek/Bridge wire 与 HTTP zero-egress 聚焦测试通过。最终本地运行 `cargo fmt -- --check`、`cargo test --locked`、
  `cargo clippy --locked -- -D warnings` 与 `git diff --check`，全部通过。
- 本轮未执行真实 Provider、外部 SDK、Models HTTP probe、负载或长期运行验收，也未读取或修改私有 credential。

2026-08-09 当前实际接入清单与文字矩阵：

- 按当前源码重新核对 31 个 canonical Model、34 个固定 Target 和 22 个 Public Model；文字矩阵候选固定为 16 个 Chat/Responses
  Generation Public Model，另外 4 个专用音频和 2 个 Embeddings Public Model 明确排除；
- 当次扩展 Models 返回 21 个可执行模型，其中 16 个文本模型、4 个专用音频模型和 1 个 Embeddings 模型；静态
  `text-embedding-3-small` 没有进入运行时 Models；
- `cargo test --locked --test example_config`：通过（24 项）；`cargo test --locked --test embedding_registry_contract`：通过（2 项）；
  `cargo test --locked --test forwarding_contract models::`：通过（2 项）；
- 对当前 checkout 构建并启动本地服务，使用私有下游用户 key 和现有 Provider credential 执行 128 个真实请求：124 个 HTTP 200
  均有非空文字和完整 JSON/SSE 终态；Spark `none` 四项返回 HTTP 400 `unsupported_value`；没有最终 429/5xx、传输或协议错误；
- 15 个简单 prompt 下缺少 reasoning 证据的 GPT `high` 单元经多步求解 prompt 定向复测后有 13 个出现 reasoning；Spark Chat
  JSON/SSE 仍无可观察 summary，而同模型 Responses 两项有 reasoning item/token；
- 本次没有运行外部 SDK、强制 fallback、负载、并发稳定性或长期运行测试；完整逐单元证据见
  [最新真实 E2E 结果](../real-e2e-test-2026-08-08.md)。

2026-08-10 已确认 `none` 能力补全：

- TDD 先固定四个 Public Model 的 Chat/Responses levels 和 Spark 的排除边界，并固定 Bailian DeepSeek Pro/Flash 的 off wire；两个
  focused test 在旧实现上均按预期失败；
- DeepSeek V4 Pro/Flash、Kimi K3 与 GLM-5.2 canonical profile 已补入 `none`；Bailian adapter 仅对两个固定 DeepSeek Chat
  deployment 将 `none` 转为 `enable_thinking:false`，不会折叠其他 effort，也不会改写 GLM；
- `cargo test --locked --test example_config`：通过（25 项）；`cargo test --locked --test provider_contract`：通过（9 项）；
  `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 全部通过；
- 本次代码修补没有重复运行 128 单元真实 Provider 矩阵、外部 SDK、强制 fallback、负载或长期测试；外部行为依据仍是上方已记录的
  E2E 与 direct off probe。

2026-08-10 Qwen3.6 27B Public Model/Route：

- 将已有 `bailian-qwen3-6-27b` Chat Target 接入下游 `qwen3.6-27b` Public Model；Chat 使用唯一 Native Route，Responses 使用同一
  Target/API 的唯一 Responses-via-Chat Bridge Route，没有声明未经验证的 Bailian Native Responses；
- 两个下游接口只公开已由 direct Chat off/on 探测确认的 `none/high`。Target reasoning output 从 `Unknown` 收窄为
  `PlainText`，Bailian adapter 将两个 level 分别转换为 `enable_thinking=false/true`，使 Responses Bridge 可以读取
  `reasoning_content`；
- Public Model 的 executable contract 仍不公开图片、视频、tools 或 Structured Outputs；canonical Model 的较宽模型事实不会扩大
  当前 Bailian Chat Target 的已确认接口能力；
- 聚焦 `qwen36_registry_contract` 在旧实现上先因 output/公开执行契约不完整而失败；实现后该契约通过（1 项），
  `example_config` 通过（25 项），`provider_contract` 通过（9 项），Models HTTP 聚焦回归通过（2 项）；
- `cargo fmt -- --check`、`cargo test --locked`、`cargo clippy --locked -- -D warnings` 与 `git diff --check` 全部通过；
- 接入后扩展 Models 单模型查询返回 HTTP 200，Chat/Responses 均投影 `none/high` 和 `plain_text`；真实下游
  `none/high × Chat/Responses × JSON/SSE` 为 8/8 HTTP 200、文字非空且终态完整，四个 `none` 均无 reasoning，四个 `high`
  均有可读 reasoning。Chat `high` 另有正 reasoning token 计数；Responses-via-Chat `high` 有 reasoning item，但未投影 reasoning
  token 计数；
- 本轮未运行外部 SDK、强制 fallback、其他 reasoning level、tools、多模态、负载、并发稳定性或长期测试；逐单元结果见
  [最新真实 E2E 结果](../real-e2e-test-2026-08-08.md)。

2026-08-10 DeepSeek V4 Flash source 优先级调整：

- `deepseek-v4-flash` 从 `NativeFirst` 切换为 `SourceFirst`，并把固定 source 顺序调整为 DeepSeek、Bailian、OpenRouter；
- Chat Route 因此按 DeepSeek、Bailian、OpenRouter 排列，Responses 因 Bailian 只有 Chat Native surface 而继续按 DeepSeek、
  OpenRouter 排列；Provider Target、能力、Bridge surface、credential pool 和 fallback 失败分类均未改变；
- route compiler 与 example-config 的相关断言已同步修改；按用户要求未执行 `cargo test`，因此本轮没有运行时测试通过证据；
- `cargo fmt -- --check` 与 `git diff --check`：通过；未运行 build、clippy、真实 Provider、外部 SDK、强制 fallback、负载或长期验收。

2026-08-11 Gemma 4 31B OpenRouter 接入确认：

- 新增 `google/gemma-4-31b-it` canonical Generation Model、`openrouter-gemma-4-31b-it` 固定 Target 和同名 Public Model；
  Chat 与 Responses 都只有 OpenRouter Native candidate，上游模型固定为 `google/gemma-4-31b-it:free`；
- 根据 2026-08-10 定向实测只公开文本/PNG 图片输入、parallel tool calls 与 `json_object`；未确认 reasoning，strict JSON Schema
  因返回 markdown 包裹内容而不公开；
- `cargo test --locked --test example_config`：通过（29 项）；provider/native routing 聚焦契约：通过（41 + 8 + 9 项）；
  `cargo test --locked`、`cargo clippy --locked -- -D warnings`、仓库级 `cargo fmt -- --check` 与 `git diff --check`：通过；
- 本轮没有重复运行真实 OpenRouter、外部 SDK、fallback、负载或长期验收；上游行为边界仍以 2026-08-10 定向请求为准。

## 相关文档

- [功能需求：Model 目录与 Provider 接入配置](../../functional-requirements/model-catalog-configuration.md)
- [Native 图片输入](../../functional-requirements/native-image.md)
- [Public Model 与能力预检](models-api-and-capability-preflight.md)
- [Provider 实施与实测状态](../providers/README.md)
- [当前代码架构](../current-architecture.md)
- [Kimi CN Provider 状态](../providers/kimi-cn.md)
- [OpenRouter 模型目录与 reasoning 复核](../../references/providers/openrouter/models.md)
- [NVIDIA Models 快照](../../references/providers/nvidia/models.md)
- [百炼 Models 快照](../../references/providers/bailian/models.md)
