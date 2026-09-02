# Alibaba Cloud Model Studio（Bailian）接入进度与边界

注册与能力事实见 `src/providers/bailian/`；当前接线见[映射](../model-provider-mapping.md)。

## 当前边界

- 2026-08-27 北京真实 Responses 对比确认基础 JSON/SSE、usage 和第一轮工具 wire 可用；在统一冲突提示下，三模型均忽略 Responses
  `text.format=json_object/json_schema`，且在单一双调用提示下均未执行 `parallel_tool_calls=false`。GLM-5.2 另有高 reasoning 400
  与标准工具续轮 arguments 类型冲突，当前继续只走 Chat bridge。
- Qwen3.8 Max、Qwen3.8 27B、Qwen3.7 Max、Qwen3.7 Plus、DeepSeek V4 Pro 0813 与 DeepSeek V4 Flash 0731 已注册双
  Native；2026-08-31 再次确认 Flash 的有界 Responses JSON/SSE，但未执行 production Router 或 SDK/Agent。
- Bailian Chat structured output 只按官方模型范围公开；Responses structured output 继续按既有差分收窄：仅
  Qwen3.7 Plus 公开 JSON Object（2026-08-11 probe 确认 `json_schema` 被静默降级），其余 Responses Target 不公开。
- Qwen/DeepSeek Responses Target 的 `parallel_calls=false` 表示不公开可精确执行 true/false 的控制，不是 serial-only 保证。
- LiveTranslate 没有下游 executable interface；Images I2I/async/stream/`b64_json` 未实现。
- Qwen/Kimi video、多图、更多图片格式/尺寸/detail、多模态 tool 组合、强制 DeepSeek fallback、其他账号/区域、质量、计费、负载与长期运行未证明。
- Bailian Responses Session cache 只表示固定 header 已进入受信 egress；cache hit、节省成本、TTL、Provider 保留策略、突发排队命中率
  及其在真实 429 下的延迟改善均未验证。

## 验证与证据

- [2026-08-27 Bailian Responses 三模型兼容性对比](../evidence/2026-08-27-bailian-responses-model-comparison.md)
- [2026-08-29 Bailian DeepSeek V4 Pro Responses 接入验证](../evidence/2026-08-29-bailian-deepseek-v4-pro-responses.md)
- [2026-08-29 Qwen3.7 Embeddings 与 Hindsight 兼容性验证](../evidence/2026-08-29-openbridge-qwen37-embeddings-hindsight-compatibility.md)（Embeddings）
- 2026-08-31 有界管理员 probe 覆盖 DeepSeek V4 Flash Responses JSON/SSE；production Router 与 SDK/Agent 未执行。

## 代码 owner

`src/providers/bailian/`（注册、媒体上限、排队/会话缓存 header 策略见代码注释）。
