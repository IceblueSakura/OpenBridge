# 语义模型调研综合

本页按问题整合历史调研，不按参考项目拆分设计。来源身份只用于追溯，不决定 OpenBridge 的模块边界。**Generation 以 OpenAI Responses 的公开语义为主要参考，再用受约束扩展承载标准之外的能力；不是 Chat/Responses 的最小公分母，也不是把某个 SDK DTO 直接作为 IR。**

- 来源：既有固定调研，来源索引见下表；当前上游快照见 [同步基线](upstream-sync.md)。
- 本次综合日期：2026-09-23；除同步基线明确列出的 OpenAI/Codex 来源外，没有重新运行或同步其他项目。
- 范围：Generation item、tools、reasoning、媒体、状态、转换与验收；OAuth grant、运营计费、MCP server 框架选型等细节保留为历史资料，不纳入当前 IR 决策。
- 边界：外部实现是经验与反例，不是公开协议标准；源码、模型质量、实际 Provider 接受、生产性能分别举证。
- 重核触发：上游 schema/SDK、扩展生命周期、目标协议或本地准入范围变化。

## 1. 表达力先于通用化

有序异构 item 比单一 messages/chat-turn 更适合作为 Generation 主干。消息、reasoning、工具调用/结果、approval、compaction 和配置变更不能被压成文本或临时 sidecar 后再猜测恢复。

参考实现共同暴露三种风险：以 Chat 为中心的多跳转换不可逆；并行维护多套 wire-shaped 核心会形成两套语义权威；过度抽象成 common content 会丢失 Responses 的 item/status/state 关系。可组合的经验是：**Responses 语义形状 + 独立受验证类型 + 有界扩展 + codec/lowering 分离**。

Responses 优先不意味着 HTTP endpoint 就是任务：Embedding、独立图片任务、ASR/TTS、voice design/clone 仍有不同任务契约。模型借 Chat wire 提供语音，不等于存在聊天历史；响应借工具返回图片，也不等于独立 Images API。

## 2. 一份语义，多个表示

同协议也必须 decode → IR → transform → validate → requirements → lower → encode。源 JSON 不能恢复已经删除的字段。跨协议转换的判据是目标是否表达最终语义，而不是能否拼出一个可解析 JSON。

等价规范化、合成身份、opaque 保留、有损变换、执行模拟和拒绝是不同结果。缺失 call ID 不用 item ID 或空字符串代替；reasoning 不默认压成 assistant text；unknown 不静默丢弃；参数不无声 clamp。源保真只记录仍有效的表示事实，不成为第二份 task payload。

## 3. 工具不是一个 name + arguments

至少分开：声明类型、调用者、执行者、调用身份、输入、结果、approval、生命周期和副作用。function/custom、标准 hosted tools、Gateway 模拟执行与 middleware 改写不能混成一类。

标准已经定义的 tool/item 应进入标准分支，而不是因为某个目标不支持就塞进 generic extension。Provider 专有 tool 才需要扩展分支。协议能够表示一个工具，不授权网关执行它；把 web search 改成 function、请求 stream 改为 non-stream 再重跑模型属于 orchestration，不属于 codec。

调用/结果、citation/source、thought signature 和 container/file reference 必须绑定正确 item、轮次与来源。执行预算和费用归属是后续 execution owner，不能借 schema 支持自动开启网络工具。

## 4. 流是状态机，不是 JSON 切片

静态 output 和 event 描述同一语义的不同时间视图。response、item、part、value、tool execution 各有结束边界；usage 和 metadata 可以晚到。transport EOF 不等于成功，错误后的正常输出不能继续。

需要保留全局 item 顺序，同时保持 summary/content 等局部索引空间。每个 response 的 materialization 要与相应静态语义一致；WebSocket 多 lane、steering 后继 response 需要外层会话 owner，不能破坏单 response 的唯一终态约束。

终态不仅用于展示最终文本，也可能决定资源归属提交、状态引用是否可 replay 和 attempt 观测。合成 stream 必须显式说明，不冒充原生增量或真实 TTFT。

## 5. 身份、缓存和状态分开

本地 ItemId/PartId、wire item ID、call ID、response ID、logical session、cache key、thread、turn-state、WebSocket lane 都不可互换。opaque reference 的可移植性需要 issuer/profile/account 或相应可信 scope 的证明。

历史实现用 transcript/cache 补 `previous_response_id`，或换目标后剥离 reasoning 的行为，只是产品恢复策略，不是标准默认语义。没有对应 owner、资源和授权时应明确拒绝，而不是“尽力兼容”。具体 Codex 扩展归属见 [扩展与上下文](extensions-and-context.md)。

## 6. 能力、执行与安全边界

标准可表达、codec 可表示、具体模型支持、endpoint 可执行、公共接口承诺是不同命题。不能因 codec 目前缺测试而宣称标准没有能力，也不能因 SDK 类型存在就宣称 Provider 支持。

runtime 的 routing、health、latency、retry、cache 与 observability 不反向定义 IR。候选独立读取最终 IR；有状态资源限制 fallback；下游已提交后不拼接第二个 attempt。来源侧的 headers、认证、endpoint 和脚本不通过 extension 自动获得执行权限。

## 7. 证据与测试吸收

独立 expected decode/encode、IR 修改/删除、拒绝与资源边界优先于 round trip。SDK consumer、Agent tool loop、协议黑盒、模型 benchmark、Provider live acceptance 和负载测试各有独立证明范围。完整方法见 [验收基线](conformance-baseline.md)。

默认只自主编写最小 synthetic fixture；参考项目的测试存在、曾经运行或许可证开源，都不意味着其 payload 可以直接复制或成为本地 oracle。

## 8. 历史来源追溯

下列材料是固定证据，不再要求先写一份来源专页才可更新主题综合。原复核日期、commit 与许可保留；本次没有把旧证据刷新为当前上游事实。

| 已整合主题 | 原始固定研究与来源边界 |
|---|---|
| item/block、Provider tool、opaque scope | [semantic types](protocol-gateways/tensorzero.md)、[content/event union](protocol-gateways/vercel-ai-sdk.md)；Apache-2.0，静态研究 |
| Responses 保真、hosted call、signature、terminal | [operation converters](protocol-gateways/bifrost.md)、[interception 回归](litellm/litellm-ir-server-tool-regressions-analysis.md)；Apache-2.0 / MIT，后者 enterprise 另有条款 |
| loss、clamp、合成流与独立 adapter | [参数转换](protocol-gateways/portkey.md)、[runtime/body 边界](protocol-gateways/helicone.md)；MIT / GPL-3.0，不复制实现 |
| hub conversion、history、call identity | [转换图](new-api/new-api-request-conversion-analysis.md)、[续轮转换](cc-switch/cc-switch-chat-responses-tool-conversion-analysis.md)、[stateful 反例](cliproxyapi/cliproxyapi-stateful-bridge-analysis.md)；AGPL-3.0 / MIT；旧 new-api focused tests 不作为当前验收 |
| 消费者字段、sidecar、容错反例 | [Agent consumer](hermes/hermes-chat-responses-analysis.md)、[SSE/tool lifecycle](codex/codex-sse-and-tool-lifecycle-analysis.md)；MIT / Apache-2.0 |
| session/cache/thread/turn | [旧 Codex 逐字段研究](codex/codex-responses-http-header-behavior.md)；最新相关源码已在同步基线重新固定 |
| 媒体任务边界 | [图片](openai/images-responses-input.md)、[文件](openai/files-responses-input.md)、[Chat audio](openai/audio-chat-input-output.md)、[特殊语音 wire](providers/xiaomi-audio.md) |
| 确定性/SDK/语义质量证据分层 | [测试资产比较](cross-project/chat-responses-sse-tool-test-suite-survey.md)、[评测方法](semantic-testing-methods.md)、[资产许可登记](topics/test-assets-registry.md) |

## 9. 到设计的映射

本页只整合研究，不定义第二套 Rust schema。接受的结构和扩展准入由 [IR 设计](../architecture-v2/semantic-ir.md)拥有；标准事实见 [Responses 标准语义](responses-standard.md)，当前代码与目标的差距见 [迁移基线](../architecture-v2/migration.md)。
