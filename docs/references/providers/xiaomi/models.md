# Xiaomi MiMo 模型目录调研

## 来源与范围

- 官方 Models list：[列出模型](https://mimo.mi.com/docs/zh-CN/api/model/list-models)（2026-08-08 抓取，页面更新时间 2026-07-17）。
- 模型下线说明：[V2.5 切换与下线](https://mimo.mi.com/docs/zh-CN/updates/deprecate)。
- OpenRouter 目录补充：[OpenRouter 模型目录](../openrouter/models.md)（2026-08-02 采集，精确匹配）。
- 语音模型能力矩阵见 [audio.md](audio.md)；协议入口见 [api.md](api.md)。

## 官方 Models list（2026-08-08）

`GET https://api.xiaomimimo.com/v1/models` 返回（`api-key` 或 `Authorization: Bearer` 认证）：

```json
{
  "object": "list",
  "data": [
    { "id": "mimo-v2.5",         "object": "model", "owned_by": "xiaomi" },
    { "id": "mimo-v2.5-asr",     "object": "model", "owned_by": "xiaomi" },
    { "id": "mimo-v2.5-pro",     "object": "model", "owned_by": "xiaomi" },
    { "id": "mimo-v2.5-tts",     "object": "model", "owned_by": "xiaomi" },
    { "id": "mimo-v2.5-tts-voiceclone",  "object": "model", "owned_by": "xiaomi" },
    { "id": "mimo-v2.5-tts-voicedesign", "object": "model", "owned_by": "xiaomi" }
  ]
}
```

- 文本生成：`mimo-v2.5-pro`、`mimo-v2.5`；语音识别：`mimo-v2.5-asr`；语音合成：`mimo-v2.5-tts`、`mimo-v2.5-tts-voicedesign`、`mimo-v2.5-tts-voiceclone`。
- 模型存在不自动证明某个 wire、参数或 OpenBridge Public Model 已可用。

## 下线模型

MiMo-V2 系列（`mimo-v2-pro`、`mimo-v2-omni`、`mimo-v2-flash`、`mimo-v2-tts`）已于 2026-06-30 00:00 正式下线，原模型名称已失效；新接入必须使用 V2.5 系列 model ID。

## OpenRouter 目录补充（2026-08-02 精确匹配）

| OpenRouter model id | `context_length` | 最大输出 | input modalities | tokenizer |
|---|---|---|---|---|
| `xiaomi/mimo-v2.5-pro` | 1,050,000 | 131,072 | `text` | `Other` |
| `xiaomi/mimo-v2.5` | 1,050,000 | 131,072 | `text, audio, image, video` | `Other` |

该表来源为 OpenRouter 目录而非 MiMo 官方页；OpenRouter 对 `mimo-v2.5` 声明 multimodal input，但官方 Models list 只返回 model id，不返回模态信息，因此模态事实以 OpenRouter 目录为准、以实际请求验收为准。

## 证据边界

Models list 只证明目录/入口事实，不能推导图片、音频、tools、reasoning、streaming、Bridge 或服务限制。动态模型目录和 Provider 行为会变化；使用前须按功能页的日期与证据层重新复核。
