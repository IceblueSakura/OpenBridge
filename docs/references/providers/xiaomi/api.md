# Xiaomi MiMo API 协议入口调研（复核于 2026-08-08）

## 来源与范围

本文只记录 Xiaomi MiMo 的公共 API origin、Chat/Responses 入口与认证事实。模型目录见 [models.md](models.md)；图片与音频协议按功能拆分：

- [模型目录与变更](models.md)
- [图片理解协议与真实观察](image.md)
- [全模型语音能力与调用途径](audio.md)

这些页面是外部 Provider 快照，不替代 OpenBridge 当前实现状态或功能需求。

- [Chat Completions API](https://mimo.mi.com/docs/zh-CN/api/chat/openai-api)
- [Responses API](https://mimo.mi.com/docs/zh-CN/api/chat/responses)
- [Models list](https://mimo.mi.com/docs/zh-CN/api/model/list-models)
- [模型下线说明](https://mimo.mi.com/docs/zh-CN/updates/deprecate)

## 观察事实

- API origin 为 `https://api.xiaomimimo.com`；Chat Completions 请求地址为 `https://api.xiaomimimo.com/v1/chat/completions`，Responses 请求地址为 `https://api.xiaomimimo.com/v1/responses`，模型列表为 `https://api.xiaomimimo.com/v1/models`。
- 认证支持两种方式（二选一，加入请求头）：
  - `api-key: $MIMO_API_KEY`
  - `Authorization: Bearer $MIMO_API_KEY`
- Responses 文档明确不支持 `background` 与 `previous_response_id`。
- Chat 使用 `thinking.type` 控制 reasoning：`enabled` 开启、`disabled` 关闭；官方关闭示例的
  `completion_tokens_details.reasoning_tokens` 为 0。
- Responses 使用标准 `reasoning.effort`，接受 `none`、`low`、`medium`、`high`。`none` 关闭 reasoning；官方明确说明
  `low`、`medium`、`high` 当前都只是开启 reasoning，行为完全相同，尚不支持细粒度强度差异。
- 旧 `mimo-v2-pro`、`mimo-v2-omni`、`mimo-v2-flash` 与 `mimo-v2-tts` 已于 2026-06-30 下线；新接入必须使用当前 model ID（见 [models.md](models.md)）。

## 证据边界

endpoint、认证与上述 reasoning 参数只证明官方协议声明，不能替代真实账号、streaming、Bridge、负载或长期运行验证。
MiMo 接受三个开启值不表示它们当前产生不同推理强度。动态模型目录和 Provider 行为会变化；使用前须按功能页的日期与证据层重新复核。
