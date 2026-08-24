# Model 与 Provider 映射

本文只记录当前 checkout 中由代码注册的 Model、Provider Target 与 Public Model 关系，不复制模型能力、上下文、模态、tokenizer、reasoning、参数或价格。能力事实以 `src/models/`、`src/providers/`、运行中的扩展 Models API 和外部官方文档为准。

## Public Model 映射

同一 Public Model 的多行按代码中的固定候选顺序排列。这里的 Provider 表示编译期 Provider family；Target ID 是运行时 Route 引用的受信上游目标。

| Public Model | Canonical Model | Provider | Target ID |
|---|---|---|---|
| `gpt-5.6-sol` | `chatgpt/gpt-5.6-sol` | ChatGPT | `chatgpt-gpt-5-6-sol` |
| `gpt-5.6-sol` | `openai/gpt-5.6-sol` | OpenAI | `openai-main` |
| `gpt-5.3-codex-spark` | `chatgpt/gpt-5.3-codex-spark` | ChatGPT | `chatgpt-gpt-5-3-codex-spark` |
| `gpt-5.5` | `chatgpt/gpt-5.5` | ChatGPT | `chatgpt-gpt-5-5` |
| `gpt-5.6-luna` | `chatgpt/gpt-5.6-luna` | ChatGPT | `chatgpt-gpt-5-6-luna` |
| `gpt-5.6-terra` | `chatgpt/gpt-5.6-terra` | ChatGPT | `chatgpt-gpt-5-6-terra` |
| `LongCat-2.0` | `meituan/longcat-2.0` | LongCat | `longcat-2` |
| `deepseek-v4-pro` | `deepseek/deepseek-v4-pro` | DeepSeek | `deepseek-v4-pro` |
| `deepseek-v4-pro` | `deepseek/deepseek-v4-pro` | Alibaba Cloud Model Studio | `bailian-deepseek-v4-pro` |
| `deepseek-v4-flash` | `deepseek/deepseek-v4-flash` | DeepSeek | `deepseek-v4-flash` |
| `deepseek-v4-flash` | `deepseek/deepseek-v4-flash` | Alibaba Cloud Model Studio | `bailian-deepseek-v4-flash` |
| `deepseek-v4-flash` | `deepseek/deepseek-v4-flash` | OpenRouter | `openrouter-deepseek-v4-flash` |
| `minimax-m3` | `minimax/minimax-m3` | OpenRouter | `openrouter-minimax-m3` |
| `minimax-m3` | `minimax/minimax-m3` | NVIDIA | `nvidia-minimax-m3` |
| `gemma-4-31b-it` | `google/gemma-4-31b-it` | OpenRouter | `openrouter-gemma-4-31b-it` |
| `kimi-k3` | `moonshotai/kimi-k3` | Kimi CN | `kimi-cn-kimi-k3` |
| `glm-5.2` | `z-ai/glm-5.2` | Alibaba Cloud Model Studio | `bailian-glm-5-2` |
| `qwen3.7-plus` | `qwen/qwen3.7-plus` | Alibaba Cloud Model Studio | `bailian-qwen3-7-plus` |
| `qwen3.7-max` | `qwen/qwen3.7-max` | Alibaba Cloud Model Studio | `bailian-qwen3-7-max` |
| `qwen3.8-max` | `qwen/qwen3.8-max` | Alibaba Cloud Model Studio | `bailian-qwen3-8-max` |
| `mimo-v2.5-pro` | `xiaomi/mimo-v2.5-pro` | Xiaomi MiMo | `mimo-v2-5-pro` |
| `mimo-v2.5` | `xiaomi/mimo-v2.5` | Xiaomi MiMo | `mimo-v2-5` |
| `mimo-v2.5-asr` | `xiaomi/mimo-v2.5-asr` | Xiaomi MiMo | `mimo-v2-5-asr` |
| `mimo-v2.5-tts` | `xiaomi/mimo-v2.5-tts` | Xiaomi MiMo | `mimo-v2-5-tts` |
| `mimo-v2.5-tts-voicedesign` | `xiaomi/mimo-v2.5-tts-voicedesign` | Xiaomi MiMo | `mimo-v2-5-tts-voicedesign` |
| `mimo-v2.5-tts-voiceclone` | `xiaomi/mimo-v2.5-tts-voiceclone` | Xiaomi MiMo | `mimo-v2-5-tts-voiceclone` |
| `text-embedding-3-small` | `openai/text-embedding-3-small` | OpenAI | `openai-text-embedding-3-small` |
| `qwen3.7-text-embedding` | `qwen/qwen3.7-text-embedding` | Alibaba Cloud Model Studio | `bailian-qwen3-7-text-embedding` |
| `nemotron-3-embed-1b` | `nvidia/nemotron-3-embed-1b` | NVIDIA | `nvidia-nemotron-3-embed-1b` |
| `qwen-image-3.0` | `qwen/qwen-image-3.0` | Alibaba Cloud Model Studio | `bailian-qwen-image-3-0` |
| `qwen-image-3.0-pro` | `qwen/qwen-image-3.0-pro` | Alibaba Cloud Model Studio | `bailian-qwen-image-3-0-pro` |

## 已注册 Target、未发布 Public Model

这些关系存在于 Provider registration，但当前没有下游 Public Model 引用对应 Target。

| Canonical Model | Provider | Target ID |
|---|---|---|
| `openai/gpt-5.5` | OpenAI | `openai-gpt-5-5` |
| `openai/gpt-5.6-luna` | OpenAI | `openai-gpt-5-6-luna` |
| `openai/gpt-5.6-terra` | OpenAI | `openai-gpt-5-6-terra` |

## 仅 Canonical Model

以下模型只存在于 `src/models/`，没有 Provider Target、Route 或 Public Model，因此不会出现在运行时 Models API。

- `deepseek/deepseek-v4-flash-vision-exp`
- `qwen/qwen-audio-3.0-asr-flash`
- `qwen/qwen-audio-3.0-realtime-flash`
- `qwen/qwen-audio-3.0-realtime-plus`
- `qwen/qwen-audio-3.0-tts-plus`
- `qwen/qwen3.8-27b`
- `z-ai/glm-5.3`

## 维护边界

- Public Model 与候选顺序：`src/providers/catalog/`。
- Provider Target 与上游 model ID：`src/providers/*/registration.rs`。
- Canonical Model 能力：`src/models/`。
- 本页只在上述注册关系变化时更新；不得复制模型能力或动态 Provider 全量目录。
