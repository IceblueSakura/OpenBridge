# Responses 上游同步基线

复核时间：**2026-09-23**。本页固定本轮设计依据，不是 Provider 实测报告或 SDK 升级完成日志。范围为公开 Responses create/item/event、reasoning、Schema、WebSocket 和 Codex 上下文相关源码。未调用模型、登录账号、读取私有配置或运行外部项目。

## 1. 权威与版本

| 证据 | 本次固定点 | 用途 |
|---|---|---|
| OpenAI 官方 API Reference / guides | 本日页面快照，URL 见下节 | 公开协议语义；动态页面不是永不变化的版本号 |
| `openai/openai-python` | [`be9d66628ad7377bd36fe5a76ae6d735843f0e76`](https://github.com/openai/openai-python/tree/be9d66628ad7377bd36fe5a76ae6d735843f0e76)，commit time `2026-09-23T06:12:58Z`，`_version.py` 为 `3.19.0` | 生成类型与可空/可选/union 的交叉证据；固定提交 LICENSE 为 Apache-2.0；不是 SDK 完整运行验证 |
| `openai/codex` | [`a69d757cd8ef8310001186865911b69e4b4175e5`](https://github.com/openai/codex/tree/a69d757cd8ef8310001186865911b69e4b4175e5)，commit time `2026-09-23T10:18:21Z` | session/header/body、turn-state 和模型 item 扩展；Apache-2.0；不是 OpenAI 公共 API 标准 |
| OpenBridge 消费者 gate | 由 [`tests/sdk/pyproject.toml`](../../tests/sdk/pyproject.toml) 与 `uv.lock` 固定 | 执行入口和局部验收范围见[开发指南](../development.md)；与本页研究快照分开维护 |
| 历史项目调研 | 各历史页原 commit、日期、许可 | 按[主题综合](semantic-baseline.md)吸收；不宣称本轮全部上游重新同步 |

官方 reference 与 guide 优先定义目标语义，SDK 用于交叉检查 required/default 与真实 consumer 形状；不同证据冲突时显式列项，不默默选择更方便实现的一份。Open Responses compliance、Codex tolerant parser 和其他 gateway 的兼容策略不是官方标准替代品。

## 2. 本轮读取的一手来源

- [Create a response](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [Streaming events](https://developers.openai.com/api/reference/resources/responses/streaming-events)
- [Reasoning guide](https://developers.openai.com/api/docs/guides/reasoning)
- [Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs)
- [WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode)
- SDK 固定源码：[`response_create_params.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_create_params.py)、[`response_input_item_param.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_input_item_param.py)、[`response_output_item.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_output_item.py)、[`response_stream_event.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_stream_event.py)、[`tool_param.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/tool_param.py)
- SDK 子域：[`response_output_message.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_output_message.py)、[`response_configuration_update_item_param_param.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_configuration_update_item_param_param.py)、[`response_input_content.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/responses/response_input_content.py)、[`reasoning.py`](https://github.com/openai/openai-python/blob/be9d66628ad7377bd36fe5a76ae6d735843f0e76/src/openai/types/shared/reasoning.py)
- Codex 固定源码：[`core/client.rs`](https://github.com/openai/codex/blob/a69d757cd8ef8310001186865911b69e4b4175e5/codex-rs/core/src/client.rs)、[`core/responses_metadata.rs`](https://github.com/openai/codex/blob/a69d757cd8ef8310001186865911b69e4b4175e5/codex-rs/core/src/responses_metadata.rs)、[`endpoint/responses.rs`](https://github.com/openai/codex/blob/a69d757cd8ef8310001186865911b69e4b4175e5/codex-rs/codex-api/src/endpoint/responses.rs)、[`requests/headers.rs`](https://github.com/openai/codex/blob/a69d757cd8ef8310001186865911b69e4b4175e5/codex-rs/codex-api/src/requests/headers.rs)、[`protocol/models.rs`](https://github.com/openai/codex/blob/a69d757cd8ef8310001186865911b69e4b4175e5/codex-rs/protocol/src/models.rs)

页面正文按相关域复核，没有把巨大的自动生成示例全部复制为 fixture。网页抽取的代码块存在重复渲染行；这些抽取伪影不能解释成 JSON 允许重复 key，也不能作为 golden data。

## 本次同步影响

| 主题 | 本次明确事实 | 对基线的影响 |
|---|---|---|
| Assistant phase | 公开 message 有 `commentary / final_answer`；要求后续 history 保留 | 标准 message 字段，不放 Codex extension；当前 codec 缺失 |
| Reasoning | 标准含 `max` effort、context、mode；configuration update 可更新后续 effort | 旧 ADR 的“不支持 max/context”不可再当当前设计；配置 item 是标准缺口 |
| Encrypted reasoning | 当前 reference/guide 明确 item added 可未完成、item done 为最终 replay；无状态 guide 说明默认包含，legacy include 仍接受 | 不以 include 或部分 snapshot 推断 finality；不能用 raw sidecar 恢复旧 token |
| WebSocket | 当前指南支持 `stream_id` 多路复用，同 lane FIFO、跨 lane 并发/fork；最多 32 named lanes，默认 lane 另计；lane ID 不属于 HTTP create | 旧“一连接仅一个 in-flight”不是现行通用规则；transport 外层与 response reducer 分开 |
| Steering / compaction | 文档描述 `steered` incomplete 后继 response，以及 compaction progress 不携带最终 summary payload | 需要 response-chain / state owner；不把第一 terminal 当整个 WS connection 结束 |
| Schema | 文档化 strict 子集、递归 refs、节点/深度约束以及按 schema key 顺序生成 | 不能仅检查 JSON object；有序 schema 与 reference graph 必须进入设计 |
| Cache | 3.19 源码有 `prompt_cache_options.prewarm`；retention deprecated 且其最大期限与 options TTL 最小期限不是同义词 | 本地执行 hints 缺新增字段；不能直接把 retention 重命名成 ttl |
| Tools/media | 公开 union 包含更多 hosted/dynamic/programmatic branches；image/file detail/source 与 tool result media 独立 | 不能把标准缺口统称特殊扩展，不能恢复 Gateway tool executor 来代替 wire 支持 |
| Codex 上下文 | 新源码仍分 logical session/cache/thread/window/turn；body canonical metadata 与 headers 是投影 | 扩展必须有 attachment/生命周期，不采用万能 session 字段 |

“本次明确”不表示全部字段都在 3.19 才引入。对本地 3.10 与本次源码进行类字段比较，`phase` 等已经在旧 SDK 中存在，但不在当前 OpenBridge codec；新增时间与实现缺口是不同事实。

## 3. 尚需按子域解决的证据差异

- SDK/事件目录有 Responses audio/transcript events，但本次 input content union 仍是 text/image/file；完整音频 request/output/terminal 联合合同需另行固定，不能由 event 名猜造。
- public response status 含 cancelled，但本次 stream event union 没有同名 `response.cancelled`；本地接受分支需分类，不把 status 自动扩展为事件。
- reference 的简写、response snapshot 与 SDK required fields 有不同上下文；完整边界不可复用宽松输入规则。SDK parse 的派生字段也不是官方 wire 语义。
- `summary:false` 是当前本地兼容行为，而本次标准 summary 是字符串枚举/null；需显式兼容 profile。
- guide 的 WS multiplex/steering 必须以 WS 事件/连接合同验收；本轮没有运行连接或将其接入当前 HTTP/SSE library。
- 本轮 SDK 检查只覆盖相关 request/item/event/source 类型，未宣称所有自动生成类、所有客户端语言与所有 API 资源都逐字段验收。

以上不是 live observed discrepancy；均为公开资料/源码对照，不建立实际 Provider 兼容性结论。

## 4. 快照可追溯性

Git 来源用完整 commit URL 固定，可重新读取精确文件。以下 SHA-256 对本轮下载原字节计算：

| 源文件 | SHA-256 |
|---|---|
| SDK `response_create_params.py` | `148f2fb0c1a51df3f03c3a83ba061beed5c9213f64109a6412010d7658cb8267` |
| SDK `response_stream_event.py` | `f1b0730198c759276a2b2ef138812c2e722eb50b4689e1368b93d16e9e83e62f` |
| Codex `core/client.rs` | `0d590b240bdbfeafacbe597a02e4bdce467f8850d597814f214184dfb89ee510` |
| Codex `core/responses_metadata.rs` | `260775242d50d913bccab678fd83f2a47bdc5c2c45b7fe0255aa04614bf8bdce` |

动态网页没有永久版本 URL；下列摘要对本轮 Tavily 清洗后的完整 Markdown 计算，不是官网原始 HTML hash，也不保证以后相同抽取器输出相同 bytes。正文及本页选择性陈述保留设计所依赖的事实；若需逐字复现，应以固定 SDK/schema 与重新归档的官方资料互证，不把 hash 当作已保存全文。

| 页面 | 清洗文本 SHA-256 |
|---|---|
| Create | `21c9691dfe0bf252a243c9c38e5572b47c74dd94aa2cb3414802120dd40e3efd` |
| Streaming events | `440bbd9678d7b1b26a015e7b14b28c81447b0f3888d4138b56c6c182268fd981` |
| Reasoning | `f4b1e5d6cdf8a4a1a4fb1be7ee02c8567b6eaea7fb89441afce02118d42ad504` |
| Structured Outputs | `21826fffd68f4fa72d952b62ea93aea046ffeef8470a168343bc1197f6145bb4` |
| WebSocket | `49025860c0ec825cc7ae70800eda109a58ecef34bb21c569ba98a4fdc824fbf4` |

## 5. 后续重核规则

新增标准/扩展子域时直接更新主题文档及本页版本，不再要求按来源新建一份调研。固定 schema/SDK/profile、列出差异和未决冲突，再更新设计与独立 fixtures；不默默升级所有消费者 gate，不把没有执行的 Provider/SDK/Agent 层写为通过。
