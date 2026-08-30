# LiteLLM IR、Responses bridge 与 server-tool interception 增量调研

## 文档元数据

| 字段 | 值 |
|---|---|
| Source snapshot | [`BerriAI/litellm` `litellm_internal_staging` @ `5e4b3838aabf00d135be800404d03728c8afa506`](https://github.com/BerriAI/litellm/tree/5e4b3838aabf00d135be800404d03728c8afa506) |
| Last reverified | 2026-08-30，本地只读源码与测试源码复核 |
| Scope | Responses→Chat bridge、reasoning/state、web-search/code-interpreter interception、stream terminal ownership、安全与测试资产 |
| Evidence boundary | 未安装依赖、未运行tests或Proxy，未调用Provider；本文记录当前源码结构与候选回归语义，不证明运行成功 |
| Recheck trigger | Responses transformer、agentic loop、interception hooks、router fallback wrapper、response ID/container ownership或license变化时 |

本文是对现有[Chat/Responses分析](litellm-chat-responses-analysis.md)和[protocol test assets](litellm-protocol-test-assets-analysis.md)的增量固定，不覆盖原有逐行快照。

## 1. Current architecture

LiteLLM仍以operation/provider-specific config和OpenAI SDK types作为主要归一层。Responses→Chat bridge由显式`use_chat_completions_api`开关选择：`litellm/types/router.py:458-462`；核心transformer位于`litellm/completion_extras/litellm_responses_transformation/transformation.py`。这不是单一protocol-neutral IR，而是Responses types、Chat message types、Provider config和bridge wrapper叠加。

当前transformer比旧快照更重视reasoning item：stored reasoning item优先，因为携带Responses-minted ID；否则从thinking blocks构造输入：`litellm/completion_extras/litellm_responses_transformation/transformation.py:94-107`。输出reasoning保留item ID、`encrypted_content`和summary：同文件 `110-171`。这是opaque state与human-readable summary应分离的正面证据，但具体字段仍是Responses-shaped。

## 2. Conversion 与 loss

bridge需要将Responses input items、tool calls/results、reasoning和terminal映射到Chat，再将Chatresponse合成为Responses item/event。可观察的转换类别不是单一exact：

- 原样保留带ID的reasoning item属于opaque preservation；
- thinking block回建reasoning item包含normalization/synthesis；
- incomplete reason被压缩到Chat `length`/`content_filter`：`litellm/completion_extras/litellm_responses_transformation/transformation.py:174-180`；
- unsupported parameters还可能走LiteLLM全局drop-params策略；
- Provider-specific item经Chat中间层可能无法无损恢复。

LiteLLM没有统一`Exact/Equivalent/Lossy/Unsupported`结果对象；转换策略分散在transformer、Provider config、warnings和hooks。它更适合作为loss case来源，不适合作为fidelity model模板。

## 3. Server-side tool interception

web-search interception是重要的实现样本：

1. pre-call识别native search tool并改写为`litellm_web_search` function tool；
2. 若客户端请求stream，会将`stream=True`改成`False`并设置内部控制字段：`litellm/integrations/websearch_interception/handler.py:332-401`、`479-486`；
3. 模型输出普通`function_call`，转换器解析其call ID和arguments：`litellm/integrations/websearch_interception/transformation.py:68-127`；
4. Gateway调用search backend；
5. Responses surface追加原function call和关联`function_call_output`：`litellm/integrations/websearch_interception/handler.py:1066-1099`；
6. 重新调用模型生成最终答案：同文件 `1606-1614`附近；
7. agentic loop受`max_agentic_loops`限制，默认3：`litellm/integrations/websearch_interception/handler.py:128-145`。

code-interpreter interception采用相似机制并维护sandbox key/session scope：`litellm/integrations/code_interpreter_interception/handler.py:217-220`、`469-475`，内部状态字段列入`agentic_loop_internal_litellm_params`以避免泄漏给Provider：`litellm/types/utils.py:3494-3506`。

这已经不只是protocol encoding：它改变stream mode、发起外部副作用、添加conversation items并重跑模型。适合抽象成独立Tool Execution Orchestrator；不应藏在encoder或Provider adapter。

## 4. Identity、state 与 security

Responses state会跨越response ID、container ID、Provider state和Proxy tenant ownership。`tests/test_litellm/test_responses_id_security.py:1-5`明确以“用户B不能读取用户A响应”为安全合同，并覆盖user/team隔离与ID加解密。部分ID encryption tests在当前快照标为flaky skip：同文件 `114-168`，因此不能把测试存在误报为完整保护已验证。

`tests/test_litellm/test_responses_streaming_container_ownership.py:1-18`记录一个真实失败类别：Router fallback wrapper未保存terminal `response.completed`，导致container ownership hook静默跳过，后续file lookup返回403。测试覆盖completed/incomplete/failed terminal、non-terminal不得设置completed response、first terminal wins，以及缺失terminal产生warning：同文件 `102-176`、`205-254`。

这说明Event IR materialization不仅服务最终JSON；terminal event还可能触发资源ownership和安全状态提交。opaque state必须声明owner、tenant、Provider/Target affinity和commit lifecycle。

## 5. Streaming

interception为了执行工具会把stream请求降为non-stream，然后可能重新包装下游stream；这属于明显的synthesized/lossy行为，必须可观察。普通Responses stream还经过Router fallback wrapper；wrapper复制iterator属性并捕获terminal object供后置hook使用。

可吸收的不变量：

- completed、incomplete、failed分别是terminal；
- non-terminal tool/content event不得触发资源commit；
- first terminal wins只是一种defensive策略，严格协议层还应拒绝duplicate terminal；
- EOF without terminal不得触发ownership成功；
- wrapper不能丢失terminal response和state-affine IDs；
- retry/fallback之后的stream owner必须绑定实际attempt。

## 6. Edge cases 与测试资产

当前快照新增或强化的候选：

1. Responses reasoning ID/summary/encrypted content往返；
2. incomplete reason映射时保持unsupported distinction，避免所有失败压成length；
3. intercepted native web search变成function call/result后，call ID和arguments保持；
4. 多个search call并发执行、result顺序与call identity；
5. `max_agentic_loops`耗尽时返回明确terminal/error；
6. interception internal control fields不得进入Provider body；生产注册位置见`litellm/types/utils.py:3491-3510`，行为回归见`tests/test_litellm/test_utils.py:5030-5057`；
7. 原stream被降成non-stream时必须声明synthesized，下游cancel与usage如何处理；
8. terminal丢失不得静默跳过container/opaque-state ownership；
9. response ID跨user/team replay必须拒绝；
10. Provider search、Gateway search和client function tool三者不能只靠相同name推断执行者。

LiteLLM主体MIT、`enterprise/`另有条款。默认只借鉴测试结构并自主写fixture；不复制enterprise代码或大段Provider transcript。

## 7. Lessons

### Adopt

- 从真实issue提炼stream terminal、tool identity、state ownership和internal-field leakage回归；
- reasoning summary、encrypted content和Provider-minted ID分离；
- server-tool loop设置有界iteration并显式关联call/result。

### Adapt

- 将interception拆成policy injection、semantic tool declaration、execution、result injection、model rerun和response projection；
- `max_agentic_loops`、budget、network/data policy、cancel和fallback形成typed contract；
- terminal触发的ownership/affinity commit进入独立lifecycle owner。

### Avoid

- 在hook中静默把stream改为non-stream；
- 用Chat作为所有Responses/Provider-native semantic的隐式IR；
- drop params或compatibility fallback后不报告loss；
- 内部control field依赖“参数构造器恰好过滤”；
- 将flaky skipped security test当作已验证保护。

### Open Questions

- interception后的usage/cost如何合并首次model call、tool执行和rerun；
- tool执行后Provider fallback是否允许，state和结果能否跨Target replay；
- cancel在search/backend/model rerun各阶段如何传播；
- synthetic stream如何表达原请求被buffered及真实TTFT；
- response/container ownership应在何种terminal与durable write顺序提交。
