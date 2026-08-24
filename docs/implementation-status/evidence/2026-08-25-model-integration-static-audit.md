# 2026-08-25 全模型接入静态审计

## 记录类型

本记录是对当前 checkout 的**静态代码、配置可用性和确定性合同测试审计**，不是全量真实 Provider 网络探测。文中的“可用”只表示当前 credential 配置能保留至少一条 executable Route；不表示上游此刻可达、账号 entitlement、配额、模型 ID 或长期稳定性已经通过网络验证。

## 审计边界

- 日期：2026-08-25，Asia/Shanghai。
- Commit：`63cb1c23c5d0fa147d2a64842d6b65dc6435b620`。
- 源码范围：`src/models/`、`src/providers/`、`src/registry/`。
- 文档对照：`docs/implementation-status/model-provider-mapping.md`、`current-state.md`、`current-boundaries.md`。
- 确定性验证：Provider contract、OpenRouter 新模型 focused forwarding contract、registry configuration availability。
- 未读取或记录 credential 值；availability report 只使用 OpenBridge 自身生成的脱敏 pool/Target/Public Model 状态。
- 未对全部 active Target 发送真实网络请求。

## 汇总结论

| 项目 | 数量 | 结论 |
|---|---:|---|
| Canonical Model | 39 | 全部存在于 `src/models/` 并被 mapping 分类 |
| 静态 Upstream Target | 39 | 34 个配置激活，5 个 OpenAI Target 未激活 |
| Public Model | 30 | 当前配置下 29 个可执行、1 个不可执行 |
| Public Model 对应 canonical | 31 | `gpt-5.6-sol` 聚合 ChatGPT 与 OpenAI 两个 canonical |
| 有 Target、无 Public Model | 3 | OpenAI GPT-5.5/Luna/Terra |
| 仅 canonical | 5 | 四个 Qwen Audio 3.0 profile 与 GLM-5.3 |
| mapping 引用未知 canonical | 0 | 未发现 |
| 未被 mapping 分类的 canonical | 0 | 未发现 |

## Provider 配置可用性

OpenBridge 在加载私有配置、编译 registry 并验证 credential binding 后生成如下 configuration-only 结果：

| Provider | 配置状态 | Target |
|---|---|---:|
| Alibaba Cloud Model Studio / Bailian | 可用 | 11/11 |
| ChatGPT | 可用 | 5/5 |
| DeepSeek | 可用 | 3/3 |
| Kimi CN | 可用 | 1/1 |
| LongCat | 可用 | 1/1 |
| Xiaomi MiMo | 可用 | 6/6 |
| NVIDIA | 可用 | 2/2 |
| OpenRouter | 可用 | 5/5 |
| OpenAI | 不可用：没有 active credential pool | 0/5 |

该 availability report 明确标注 `no network probe`。临时进程随后因当前 shell 用户无权创建配置的 HTTP JSONL 目录而停止：`failed to create HTTP JSONL directory: Permission denied`。因此本记录不声称该临时进程成功监听端口。

## Generation Public Models

`Native` 表示直接上游协议；`Bridge` 表示 OpenBridge 在 Chat/Responses 之间执行已建模转换。Target 按固定候选顺序列出。

| Public Model | 固定 Target 顺序 | 协议接入 | 配置状态 |
|---|---|---|---|
| `gpt-5.6-sol` | `chatgpt-gpt-5-6-sol` → `openai-main` | ChatGPT Responses Native + Chat Bridge；OpenAI 双 Native | 可用；OpenAI 候选未激活 |
| `gpt-5.3-codex-spark` | `chatgpt-gpt-5-3-codex-spark` | Responses Native + Chat Bridge | 可用 |
| `gpt-5.5` | `chatgpt-gpt-5-5` | Responses Native + Chat Bridge | 可用 |
| `gpt-5.6-luna` | `chatgpt-gpt-5-6-luna` | Responses Native + Chat Bridge | 可用 |
| `gpt-5.6-terra` | `chatgpt-gpt-5-6-terra` | Responses Native + Chat Bridge | 可用 |
| `LongCat-2.0` | `longcat-2` | Chat/Responses 双 Native，并保留显式 Bridge surface | 可用 |
| `deepseek-v4-pro` | `deepseek-v4-pro` → `bailian-deepseek-v4-pro` | DeepSeek 双 Native；Bailian Chat Native + Responses Bridge | 可用 |
| `deepseek-v4-flash` | `deepseek-v4-flash` → `bailian-deepseek-v4-flash` → `openrouter-deepseek-v4-flash` | DeepSeek/OpenRouter 双 Native；Bailian Chat Native + Responses Bridge | 可用 |
| `deepseek-v4-flash-vision-exp` | `deepseek-v4-flash-vision-exp` | Chat/Responses 双 Native、图片输入 | 可用 |
| `minimax-m3` | `openrouter-minimax-m3` → `nvidia-minimax-m3` | OpenRouter 双 Native；NVIDIA Chat Native + Responses Bridge | 可用 |
| `gemma-4-31b-it` | `openrouter-gemma-4-31b-it` | Chat/Responses 双 Native | 可用 |
| `gemini-3.7-flash` | `openrouter-gemini-3-7-flash` | Chat/Responses 双 Native、图片输入 | 可用 |
| `grok-4.6` | `openrouter-grok-4-6` | Chat/Responses 双 Native、图片输入 | 可用 |
| `kimi-k3` | `bailian-kimi-k3` → `kimi-cn-kimi-k3` | 两个 Chat Native Target；Responses 由 Chat Bridge 补齐 | 可用 |
| `glm-5.2` | `bailian-glm-5-2` | Chat Native + Responses Bridge | 可用 |
| `qwen3.7-plus` | `bailian-qwen3-7-plus` | Chat/Responses 双 Native、图片输入 | 可用 |
| `qwen3.7-max` | `bailian-qwen3-7-max` | Chat/Responses 双 Native、text-only | 可用 |
| `qwen3.8-max` | `bailian-qwen3-8-max` | Chat/Responses 双 Native、图片输入 | 可用 |
| `qwen3.8-27b` | `bailian-qwen3-8-27b` | Chat/Responses 双 Native、图片输入 | 可用 |
| `mimo-v2.5-pro` | `mimo-v2-5-pro` | Chat/Responses 双 Native、text-only | 可用 |
| `mimo-v2.5` | `mimo-v2-5` | Chat/Responses 双 Native；Chat 支持图片/WAV，Responses 支持图片 | 可用 |
| `mimo-v2.5-asr` | `mimo-v2-5-asr` | 专用 Chat Native，无 Bridge | 可用 |
| `mimo-v2.5-tts` | `mimo-v2-5-tts` | 专用 Chat Native，无 Bridge | 可用 |
| `mimo-v2.5-tts-voicedesign` | `mimo-v2-5-tts-voicedesign` | 专用 Chat Native，无 Bridge | 可用 |
| `mimo-v2.5-tts-voiceclone` | `mimo-v2-5-tts-voiceclone` | 专用 Chat Native，无 Bridge | 可用 |

## Embeddings 与 Images Public Models

| Public Model | Target | Operation | 配置状态 |
|---|---|---|---|
| `text-embedding-3-small` | `openai-text-embedding-3-small` | Embeddings Native | 不可用：OpenAI pool 未激活 |
| `qwen3.7-text-embedding` | `bailian-qwen3-7-text-embedding` | Embeddings Native | 可用 |
| `nemotron-3-embed-1b` | `nvidia-nemotron-3-embed-1b` | Embeddings Native | 可用 |
| `qwen-image-3.0` | `bailian-qwen-image-3-0` | Images Generations Native | 可用 |
| `qwen-image-3.0-pro` | `bailian-qwen-image-3-0-pro` | Images Generations Native | 可用 |

Images 当前只接入同步 generations；image edit、异步、streaming 和 `b64_json` 不在 executable surface 中。Canonical 对模型编辑能力的描述不能替代独立的 edit operation 注册。

## 已注册 Target、未发布 Public Model

以下 canonical 有 OpenAI Target，但没有 Public Model 引用；即使以后激活 OpenAI pool，也不会自动出现在下游 Models API：

- `openai/gpt-5.5` → `openai-gpt-5-5`
- `openai/gpt-5.6-luna` → `openai-gpt-5-6-luna`
- `openai/gpt-5.6-terra` → `openai-gpt-5-6-terra`

## 仅 Canonical Model

以下模型没有 Provider Target、Route 或 Public Model：

- `qwen/qwen-audio-3.0-asr-flash`
- `qwen/qwen-audio-3.0-realtime-flash`
- `qwen/qwen-audio-3.0-realtime-plus`
- `qwen/qwen-audio-3.0-tts-plus`
- `z-ai/glm-5.3`

Qwen Audio profile 不能仅凭 canonical facts 伪装成普通 Chat Completions；仍需独立 DashScope native/Realtime 协议实现。

## 多模态静态接入摘要

- DeepSeek Vision：Chat/Responses Native；JPEG/PNG/GIF/WebP；remote/data URL；最多 600 image parts；支持 `auto/low/high/original` detail。
- OpenRouter Gemini/Grok：Chat/Responses Native；JPEG/PNG；remote/data URL；最多 4 image parts。
- Bailian Qwen：`qwen3.7-plus`、`qwen3.8-max`、`qwen3.8-27b` 双 Native图片；公开 BMP/JPEG/PNG/TIFF/WebP/HEIC 与 250-part 共同上限。
- Bailian/Kimi CN：`kimi-k3` Chat 图片能力参与保守聚合；Responses Bridge 不提供图片等价转换。
- Xiaomi MiMo：`mimo-v2.5` 双 Native 图片，Chat 另有 WAV 音频理解；四个专用音频 task 为 Chat-only。
- ChatGPT：Responses Native profile 有 typed image input；Chat Bridge 不提供 image Bridge。
- 所有当前 executable Target 都关闭 file input、Files API、video 和未建模的 media Bridge。

## 审计发现

### OpenRouter MiniMax/Gemma Chat 图片证据缺口

`src/providers/openrouter/registration.rs` 当前把 Chat `IMAGE_INPUT` 应用于除 DeepSeek Flash 外的全部 OpenRouter Targets，因此 MiniMax M3、Gemma 4、Gemini 3.7 Flash 和 Grok 4.6 都获得 Chat image profile。

但 `src/providers/openrouter/media.rs` 的当前证据注释和 focused forwarding test 只明确覆盖 Gemini/Grok。仓库没有找到 OpenRouter MiniMax/Gemma 图片请求的聚焦合同：

- Gemini/Grok 图片合同有对应 probe 说明和确定性 forwarding test；
- OpenRouter MiniMax/Gemma Chat image 已在 executable contract 中公开，但当前证据链不完整；
- 后续应真实探测 MiniMax/Gemma 并补测试，或在验证前把 OpenRouter image narrowing 限定为 Gemini/Grok。

NVIDIA media 注释记录了 2026-08-10 对 generation family 的单张 PNG data URL 成功观察；JPEG 仅依据 OpenAI-compatible endpoint convention 声明。该证据不等于 OpenRouter MiniMax 图片验证。

### 状态文档未穷举全部 image interface

`current-state.md` 的 Native 图片摘要重点列出 DeepSeek Vision、OpenRouter Gemini/Grok、Bailian 和 MiMo，但没有穷举 ChatGPT Responses、OpenRouter Gemma Chat 和 MiniMax Chat 的当前静态 interface。字段级事实仍应以 registry source 与运行中的扩展 Models API 为准。

## 已执行验证

```text
cargo test --locked --test provider_contract
cargo test --locked --test forwarding_contract \
  native::openrouter_new_models_expose_probed_dual_native_image_and_reasoning_contracts -- --exact
```

结果：

- Provider contract：12 passed；
- OpenRouter focused forwarding：1 passed；
- Git 工作树在审计开始时为 clean `main...origin/main`；
- 39 个 canonical 与 mapping 的分类交叉检查：未知 0、遗漏 0。

## 不证明范围

本记录不证明：

- 34 个 active Target 在 2026-08-25 的实时网络可达性；
- 当前账号 entitlement、模型目录、配额或计费状态；
- 任何模型 ID 此刻仍可真实生成；
- 全部 tool、structured output、reasoning、stream、图片和多能力组合；
- 外部 SDK/Agent 兼容性；
- 负载、视觉/音频质量、恶意媒体处理或长期稳定性。

需要这些结论时，应使用精确 Target、固定 payload 和有界 token 参数执行新的真实 Provider probe，并另建带日期 evidence 记录。