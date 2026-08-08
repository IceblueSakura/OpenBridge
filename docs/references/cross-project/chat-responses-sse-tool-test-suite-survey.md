# Chat/Responses、SSE 与工具调用测试资产综合调研

## 1. 状态与项目级前置文档

在线复核日期：2026-07-26。未固定 commit 的项目必须在实际采用、复制或运行前重新 pin 版本并核对许可证。

本综合文档只比较以下独立调研结果：

- [OpenAI gpt-oss compatibility-test](../openai/gpt-oss-compatibility-test-analysis.md)
- [Open Responses Compliance](../openai/open-responses-compliance-analysis.md)
- [Codex Responses 与工具生命周期 tests](../codex/codex-protocol-test-assets-analysis.md)
- [LiteLLM Responses 与转换 tests/issues](../litellm/litellm-protocol-test-assets-analysis.md)
- [OpenAI SDK streaming consumers](../openai/openai-sdk-stream-test-assets-analysis.md)

## 2. 评估维度

| 维度     | 需要区分的问题                                                                   |
|----------|----------------------------------------------------------------------------------|
| 协议方向 | Native Chat、Native Responses、Responses → Chat、Chat → Responses 是否分别覆盖   |
| 确定性   | 固定 transcript，还是依赖真实模型按 prompt 选择行为                              |
| 流式粒度 | 最终对象、semantic event、SSE framing 与任意 bytes 分片分别覆盖到哪一层          |
| 工具身份 | `call_id`、item id、choice/output index、name 与 arguments 如何关联              |
| 终态     | completed、failed、incomplete、`[DONE]`、EOF、cancel 与 transport error 是否区分 |
| 状态     | continuation、store、reconnect 与 route/account affinity 是否覆盖                |
| 集成性   | 能否离线运行、是否依赖 SDK、真实模型、项目内部类型或数据库                       |

只比较最终文本或最终 JSON，无法证明 streaming conversion 的 event 顺序、tool identity 或 terminal 正确。

## 3. 覆盖比较

`强` 表示资产直接覆盖；`部分` 表示可提取场景或做互证；`无` 表示不能从现有资产推导。

| 测试资产                    | Chat | Responses |   Bridge | SSE 语义 | 复杂 tools | fault/cancel | 确定性 |
|-----------------------------|-----:|----------:|---------:|---------:|-----------:|-------------:|-------:|
| gpt-oss compatibility-test  | 部分 |      部分 |       无 |     部分 |       部分 |           无 |     弱 |
| Open Responses Compliance   |   无 |        强 |       无 |     部分 |         弱 |         部分 |     中 |
| Codex tests                 |   无 |        强 |       无 |       强 |         强 |         部分 |     强 |
| LiteLLM tests/issues        | 部分 |        强 |     部分 |       强 |         强 |         部分 |     中 |
| OpenAI SDK consumer tests   |   强 |        强 |       无 |     部分 |       部分 |           弱 |     中 |

“确定性弱”只表示结果受模型采样或远端服务影响，不是项目质量评价。

## 4. 跨资产结论

没有一个公开套件同时覆盖：

- Chat/Responses 双向转换；
- non-stream 与 SSE；
- UTF-8/CRLF/multiline data 和任意 bytes 分片；
- 单个、连续、并行 function calls；
- index/id/name/arguments 的独立分片与关联；
- tool result、continuation 与 state affinity；
- completed/failed/incomplete、EOF、cancel、HTTP 200 内错误；
- 不支持字段和输出后的失败边界。

不同资产的证据角色也不同：

- gpt-oss compatibility-test 与 OpenAI SDK consumer tests 主要是 SDK/API-shape smoke；
- Open Responses Compliance 主要是 Responses 黑盒 schema、terminal 与 continuation；
- Codex tests 主要是 Responses client tool lifecycle；
- LiteLLM tests/issues 主要提供多 Provider 转换差异和负面回归；
- SDK accumulator 主要说明客户端如何消费增量。

## 5. 适用边界

- 外部 schema smoke 不等于跨协议状态机 contract。
- 真实模型测试不适合作为逐 commit 的确定性 oracle。
- 项目内部 cache 补全、静默丢弃或合成 identity 都是项目策略，不自动成为通用协议语义。
- fixture 被复制或改写时需要记录来源 commit、许可证和修改内容。
- 任何“兼容”结论都必须明确具体 SDK/项目版本、endpoint、stream mode、tool 场景与失败边界。
