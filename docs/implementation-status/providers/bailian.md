# Alibaba Cloud Model Studio（Bailian）接入进度与边界

注册与能力事实见 `src/providers/bailian/`；当前 Target 与 Public Model 关系见[映射](../model-provider-mapping.md)。

## 特有接线与例外

- Qwen3.8 Max、Qwen3.8 27B、Qwen3.7 Max、Qwen3.7 Plus、DeepSeek V4 Pro 0813 与 DeepSeek V4 Flash 0731 注册为双 Native；Kimi K3 为 Chat-only，图片 Targets 为 Images Generations。
- 2026-08-27 的北京 Responses 对比确认三模型基础 JSON/SSE、usage 和第一轮工具 wire；统一冲突提示下三模型均忽略 `text.format=json_object/json_schema`，且未执行 `parallel_tool_calls=false`。GLM-5.2 另有高 reasoning 400 与标准工具续轮 arguments 类型冲突，当前继续只走 Chat bridge。
- Bailian Chat structured output 按官方模型范围公开；Responses structured output 继续收窄：仅 Qwen3.7 Plus 公开 JSON Object，其他 Responses Target 不公开。Qwen/DeepSeek Responses 的 `parallel_calls=false` 不是 serial-only 保证。
- Responses Session cache 只证明固定 header 进入受信 egress，不证明 cache hit、TTL、节省成本、Provider 保留策略或 429 下延迟改善。
- LiveTranslate 没有下游 executable interface；Images I2I、async、stream、`b64_json` 未实现。其他账号/区域、质量、计费、负载和长期运行不在这些记录覆盖内。

## 验证与证据入口

- [2026-08-27 Bailian Responses 三模型兼容性对比](../evidence/2026-08-27-bailian-responses-model-comparison.md)
- [2026-08-29 Bailian DeepSeek V4 Pro Responses 接入验证](../evidence/2026-08-29-bailian-deepseek-v4-pro-responses.md)
- [2026-08-29 Qwen3.7 Embeddings 与 Hindsight 兼容性验证](../evidence/2026-08-29-openbridge-qwen37-embeddings-hindsight-compatibility.md)（Embeddings）
- [2026-09-02 双协议能力探测矩阵](../evidence/2026-09-02-dual-protocol-capability-matrix.md)（Qwen3.8 Max）
- 2026-08-31 有界管理员 probe 覆盖 DeepSeek V4 Flash Responses JSON/SSE；未形成 production Router 或 SDK/Agent 验收。

## 代码 owner

`src/providers/bailian/`（注册、媒体上限、排队/会话缓存 header 策略见代码注释）。
