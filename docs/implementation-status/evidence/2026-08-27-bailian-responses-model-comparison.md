# 2026-08-27 Bailian Responses 三模型兼容性对比

## 来源声明

- Bailian OpenAI-compatible Responses 文档：<https://help.aliyun.com/zh/model-studio/qwen-api-via-openai-responses>。
- Bailian GLM 模型页：<https://help.aliyun.com/zh/model-studio/glm>。
- Bailian DeepSeek API 文档：<https://help.aliyun.com/zh/model-studio/deepseek-api>。
- DeepSeek V4 Flash 模型页：<https://help.aliyun.com/zh/model-studio/deepseek-v4-flash>。
- Qwen3.8 Max 模型页：<https://help.aliyun.com/zh/model-studio/qwen3-8-max>。
- 官方兼容性说明明确指出：仅处理文档列出的参数，未列出的 OpenAI 参数可能被忽略；`background` 当前不支持。

## 测试环境与范围

- 时间：2026-08-27，Asia/Shanghai。
- 仓库基线：`44765ae64c348ea8bf05d93a1351bebd0f5d8b92`；测试前工作区干净。
- Provider/region：Alibaba Cloud Model Studio（Bailian），华北 2（北京）公共 OpenAI-compatible endpoint。
- Endpoint：`POST https://dashscope.aliyuncs.com/compatible-mode/v1/responses`。
- Model：`glm-5.2`、`deepseek-v4-flash-0731`、`qwen3.8-max`。
- Credential：读取既有 `bailian-primary` pool；本文不保存或复制 key、账户标识、Workspace ID、认证 header 或 Provider request ID。
- Payload：短合成文本、合成 function schema 和合成 tool output；不包含真实用户数据。
- 可复现性缺口：执行时使用的临时命令、client/package 版本与脚本未写入仓库，因此本文只能作为当日观察记录，不能从 checkout 独立重放完整矩阵。
- 原始响应正文、完整 reasoning、stream body 和 state ID 未写入仓库；本文只保留脱敏后的 wire 形状、状态、计数与结论。

## 上游 Models API 可见性快照

同一北京 credential 对以下入口执行只读查询：

```text
GET https://dashscope.aliyuncs.com/compatible-mode/v1/models
```

结果为 HTTP 200、总模型数 242；按 model ID 包含 `glm` 过滤得到 9 个可见 ID：

```text
glm-4.7
glm-5
glm-5.1
glm-5.2
glm-5.2-fast-preview
ZHIPU/GLM-5
ZHIPU/GLM-5.1
ZHIPU/GLM-5.2
ZHIPU/GLM-5.3
```

当前目录没有 `glm-5.3-flash` 或 `ZHIPU/GLM-5.3-Flash`；`ZHIPU/GLM-5.3` 是不同的非 Flash 模型，不能作为 GLM-5.3-Flash alias。Models API 可见性只证明当前 endpoint 返回目录项，不替代真实 generation entitlement；本记录随后只对 `glm-5.2`、`deepseek-v4-flash-0731` 与 `qwen3.8-max` 执行 generation/Responses probe。

能力状态采用以下术语：

- `accepted`：HTTP 请求成功，但不证明参数生效。
- `observed conformant`：本次输出符合约束，但未通过冲突差分证明字段本身生效。
- `enforced`：冲突差分显示字段改变了输出行为。
- `rejected`：本次 request shape 被明确拒绝；不自动外推为永久或全模型 unsupported。
- `observed ignored`：请求被接受，但冲突差分显示受测字段未约束本次输出；不等于长期稳定，也不外推到其他 request shape。
- `Provider-wide`：本次三个模型均出现相同行为；不外推到未测试的其他 Bailian 模型。
- `model-specific`：相同 endpoint、credential 和请求形状下，仅某个模型出现。

## 统一结果矩阵

| 能力 | `glm-5.2` | `deepseek-v4-flash-0731` | `qwen3.8-max` | 归因 |
|---|---|---|---|---|
| Non-streaming text | 200 / `completed` | 200 / `completed` | 200 / `completed` | 三模型可用 |
| Streaming text | 200 / `response.completed` | 200 / `response.completed` | 200 / `response.completed` | 三模型可用 |
| SSE `[DONE]` | 无 | 无 | 无 | 三模型共同观察 |
| Terminal usage | 完整 | 完整 | 完整 | 三模型共同观察 |
| Reasoning item | `summary_text` | `summary_text` | `summary_text` | 三模型共同观察 |
| `reasoning=none` | 200 | 200 | 200 | 三模型可用 |
| `reasoning=high` | 400 | 200 | 200 | GLM-specific failure |
| `text.format=json_object` | 200，但忽略 | 200，但忽略 | 200，但忽略 | 三模型共同观察（仅该格式） |
| `text.format=json_schema` | 200，但忽略 | 200，但忽略 | 200，但忽略 | 三模型共同观察（仅该格式） |
| 第一轮 non-stream function call | 成功 | 成功 | 成功 | 三模型可用 |
| 第一轮 streaming function call | 成功 | 成功 | 成功 | 三模型可用 |
| Standard stateless tool continuation | 400 | 成功 | 成功 | GLM-specific failure |
| Standard stateful tool continuation | 400 | 成功 | 成功 | GLM-specific failure |
| `parallel_tool_calls=false` | 仍返回两个 calls | 仍返回两个 calls | 仍返回两个 calls | 三模型共同观察（单一双调用提示） |
| `strict:true` function schema | accepted / observed conformant | accepted / observed conformant | accepted / observed conformant | 未证明 enforced |
| Text state with `store=true` | 成功 | 未独立重复；stateful tool 成功 | 未独立重复；stateful tool 成功 | 上游支持 state |
| `store=false` 后 previous ID | 400 Not found | 未独立重复 | 未独立重复 | GLM 实测，符合字段语义 |
| `background=true` | 400 | 未逐模型重复 | 未逐模型重复 | 官方通用声明；仅 GLM live 验证 |
| 未知字段 | 200 且忽略 | 未逐模型重复 | 未逐模型重复 | 官方通用声明；仅 GLM live 验证 |

## 基础 JSON、SSE 与 usage

三模型 non-streaming 均返回：

```text
object=response
status=completed
output=[message/output_text]
usage.input_tokens
usage.output_tokens
usage.output_tokens_details.reasoning_tokens
usage.total_tokens
```

Bailian 额外返回：

```text
usage.x_details[*].x_billing_type=response_api
```

三模型基础 SSE 顺序一致：

```text
response.created
response.in_progress
response.output_item.added
response.content_part.added
response.output_text.delta
response.output_text.done
response.content_part.done
response.output_item.done
response.completed
```

思考请求还会出现：

```text
response.reasoning_text.delta
response.reasoning_text.done
```

工具请求还会出现：

```text
response.function_call_arguments.delta
response.function_call_arguments.done
```

三模型均不发送 `data: [DONE]`。成功终态必须由 `response.completed` 判定，terminal usage 位于该事件的 `response.usage`。

## Reasoning 对比

### GLM-5.2

实测：

| 请求 | 结果 |
|---|---|
| 省略 `reasoning` | 200，不思考 |
| `none` | 200 |
| `minimal` | 200 |
| `low` | 200 |
| `medium` | 200 |
| `high` | 400 |
| `xhigh` | 400 |
| `max` | 400 |
| 非法值 `bogus` | 400，并返回合法枚举 |

`high/xhigh/max` 均返回：

```text
InternalError.Algo.InvalidParameter:
The thinking_budget parameter must be a positive integer
and not greater than 131072
```

去掉 `max_output_tokens` 后 `max` 仍失败，因此不是输出上限组合问题。`enable_thinking=false` 成功且 reasoning tokens 为 0；`enable_thinking=true` 在 64-token 请求中只生成 reasoning，耗时约 139 秒并以 `incomplete` 结束。

### DeepSeek V4 Flash 0731 与 Qwen3.8 Max

相同北京 endpoint、credential、算术提示和 `reasoning.effort=high`：

- `deepseek-v4-flash-0731`：200，`reasoning` + `message`，结果正确。
- `qwen3.8-max`：200，`reasoning` + `message`，结果正确。

该高档 reasoning failure 未在另外两个受测模型上复现；这支持将问题收窄到当前 GLM-5.2 路径，但不证明其他 Bailian Responses 模型的行为。

### Reasoning wire

三模型 non-streaming reasoning item 均为：

```json
{
  "type": "reasoning",
  "summary": [
    {
      "type": "summary_text"
    }
  ]
}
```

未观察到 `encrypted_content`。Bailian Provider 的 Responses ceiling 使用 `ReasoningOutput::Summary`，当前 executable Qwen Responses target 继承该值，与本次 Responses wire 一致；现有 GLM/Qwen/DeepSeek Chat operation 则分别使用 `ReasoningOutput::PlainText`，两种协议合同不能混写。

## Structured output 差分

统一冲突提示：

```text
Ignore any requested output format.
Reply with the plain text word OK, not JSON.
```

分别发送：

```json
{"text":{"format":{"type":"json_object"}}}
```

```json
{
  "text": {
    "format": {
      "type": "json_schema",
      "name": "answer",
      "strict": true,
      "schema": {
        "type": "object",
        "properties": {"answer": {"type": "string", "const": "OK"}},
        "required": ["answer"],
        "additionalProperties": false
      }
    }
  }
}
```

GLM 还重复测试了 `json_object`，并测试 legacy：

```json
{"response_format":{"type":"json_object"}}
```

三模型全部返回 200，但 output text 均为纯文本 `OK`，无法解析为 JSON。结论：本次测试的 `text.format=json_object` 与 `text.format=json_schema` request shape 被接受但忽略；GLM 的 legacy `response_format=json_object` 也被忽略。该证据不覆盖其他 structured-output 字段或 schema keyword，不能把这些具体结果扩写为所有变体的统一结论。

## Function tools 第一轮

统一合成工具：

```json
{
  "type": "function",
  "name": "lookup",
  "parameters": {
    "type": "object",
    "properties": {"value": {"type": "string"}},
    "required": ["value"],
    "additionalProperties": false
  },
  "strict": true
}
```

三模型在 `tool_choice=required` 下均返回标准第一轮 function call：

- `call_id` 存在；
- non-stream `arguments` 为 JSON string；
- stream argument delta、done 和 terminal item 中 `arguments` 均为 string；
- 正常样例解析为 `{"value":"alpha"}`；
- SSE 由 `response.completed` 结束，无 `[DONE]`。

`strict:true` 被接受，正常样例 observed conformant。GLM 冲突 enum 测试中，strict true/false 都拒绝非法值，未证明 strict 字段本身产生差异，因此不能标记为 enforced。

## 标准工具续轮

### DeepSeek V4 Flash 0731

以下两种标准路径均成功，并最终报告 `RESULT_ALPHA`：

1. Stateless：完整 `user + function_call + function_call_output`。
2. Stateful：`store=true` 后通过 `previous_response_id + function_call_output`。

### Qwen3.8 Max

与 DeepSeek 相同，两种标准路径均成功；function output 被正确消费，没有额外 function call。

### GLM-5.2

第一轮 function-call wire 标准，但两种标准续轮都 400：

```text
function_call.arguments
Input should be a valid string
input_value={'value': 'alpha'}
input_type=dict
```

观察到的处理链为：

1. 第一轮 wire 返回 JSON string；
2. Bailian 内部续轮路径先将它解码为 object；
3. 下一层校验器又要求 string；
4. stateless replay 与服务端保存的 stateful replay 都失败。

将 arguments 再额外 JSON 编码一层后，续轮成功并报告 `RESULT_ALPHA`。该双重编码是非标准、脆弱的 Provider workaround，不作为公共合同，也不建议直接固化为兼容 shim。

## `parallel_tool_calls` 差分

统一提示要求在一个响应中分别调用：

```text
lookup(alpha)
lookup(beta)
```

即使显式发送：

```json
{"parallel_tool_calls":false}
```

三模型仍各返回两个 function-call items，同时顶层正确回显 `parallel_tool_calls=false`。因此：

- 本次三个请求都观察到多个 function-call items；
- 字段被接受和回显；
- 在本次单一双调用提示中，`false` 没有禁止多个 calls；
- OpenBridge 不能据此声明 `parallel_calls=true`：当前该字段表示 upstream 能精确执行 `true/false` 两种控制，而本次 `false` 差分已经否定这一合同；保持 `false` 会使显式 true/false 在 egress 前 fail closed，同时不否认省略字段时可能返回多个 calls。

## State、background 与未知字段

GLM 文本 state 实测：

- `store=true` 后通过 `previous_response_id` 成功恢复合成 codeword；
- `store=false` 后 previous ID 返回 400 Not found；
- 说明 `store` 语义被执行。

DeepSeek/Qwen 的 stateful function-call continuation 成功，证明对应 state 能支持标准工具闭环。

`background=true` 在 GLM 上明确返回：

```text
400 Currently not support background.
```

这与官方 Responses 文档的通用限制一致，但 DeepSeek/Qwen 未逐模型重复该 probe。GLM 未知字段请求返回 200 并被忽略，也符合官方“只处理明确列出的参数”的 fail-open 说明；同样未对另两个模型独立验证。

OpenBridge 当前保持 `state=Stateless`、`background=false` 是保守且安全的公共合同；是否公开 stateful Responses 是独立产品决策，不能由上游可用性自动推导。

## 三模型共同观察、官方通用声明与 model-specific 归因

### 本次三模型共同观察

- 基础 non-stream/stream、terminal usage 可用；
- 以 `response.completed` 终止，不发送 `[DONE]`；
- reasoning 使用 `summary_text`；
- `text.format=json_object/json_schema` 在统一冲突提示下被静默忽略；GLM 的 legacy `response_format` 也被忽略；
- `parallel_tool_calls=false` 在统一双调用提示下被回显，但三模型仍各返回两个 calls；
- 第一轮 function-call wire 标准；

这些结论只覆盖本文列出的 request shape，不外推到未测试的 structured-output 变体、tool schema 或其他 Bailian 模型。

### 官方通用声明，仅 GLM live 验证

- `background` 不支持；
- 未声明字段可能被忽略，需要网关本地 fail-closed preflight。

### GLM-5.2 特定问题

- `high/xhigh/max` reasoning effort 在当前 endpoint 400；
- 标准 stateless/stateful function-call continuation 均因 arguments 内部类型冲突失败；
- `enable_thinking=true` 在小输出预算下可能只生成 reasoning 并 `incomplete`。

### DeepSeek/Qwen 当前实测差异

- `reasoning=high` 成功；
- 标准 stateless/stateful function-call continuation 成功；
- Native Responses text/tools 可作为接入候选；
- 仍受本次共同观察到的 structured-output request shape、parallel false 双调用形状与无 `[DONE]` 边界；background 是官方通用限制，但未逐模型重复。

## 对 OpenBridge 的接入建议

### Qwen3.8 Max

当前代码已为 `bailian-qwen3-8-max` 注册并公开 Native Responses target。本次 direct upstream 结果验证了其标准 stateless/stateful tool continuation；但没有经过 OpenBridge 本地二进制、扩展 Models API、SSE renderer 或 Hermes `obr` 运行时复验。

当前 executable target 合同为：

```text
streaming:           true
terminal_usage:      true
function_tools:      enabled
parallel_calls:      false
strict_schema:       false
structured_outputs:  None
state:               Stateless
background:          false
reasoning_output:    Summary
```

这些保守边界与本次证据一致。这里的 `parallel_calls=false` 不是 serial-only 保证，而是“不公开可精确控制并行调用”；上游在 `parallel_tool_calls=false` 时仍返回两个 calls，正好支持继续拒绝显式 true/false，而不是把该字段改为 true。

### DeepSeek V4 Flash 0731

当前 DeepSeek V4 Flash 0731 target 仍是 Chat-only。可评估新增 Native Responses operation；正式发布前仍需在 OpenBridge 本地二进制验证 parser、SSE terminal、tool loop 与 Hermes `obr`，本记录只证明 direct upstream wire。

### GLM-5.2

推荐：

```text
Chat Completions: Native Bailian
Responses:         Chat -> Responses bridge
```

不建议直接公开 Native Responses tools。若必须新增 Native Responses，只应先做 text-only，并至少关闭：

```text
function_tools
structured_outputs
state
background
```

当前公共 endpoint 实测可用 reasoning 档位为 `none/minimal/low/medium`；但 canonical model facts 与 Chat/Responses 共用 reasoning 档位，未必能按 operation 独立收窄，这进一步支持使用 Chat bridge。

对于 Bailian Responses Provider ceiling，不应仅凭 Qwen/DeepSeek 的其他能力全局公开 structured output。本记录支持三个模型的 effective Responses target 均收窄为 `None`：Qwen3.8 Max 当前已如此，DeepSeek/GLM 若新增 Native Responses operation 也应保持关闭。是否修改 Provider-wide ceiling 需要额外覆盖其他 Responses 模型后再决定。

## 验证边界与安全说明

本记录只证明：

- 单一北京账号与现有 credential；
- 公共 `dashscope.aliyuncs.com` endpoint，而非 Workspace 专属域名或新加坡 endpoint；
- 当前日期、当前模型版本和短合成 payload；
- 少量单次请求及有限差分，不构成稳定性、SLA、质量或计费精度验证。

未验证：

- 长上下文、媒体输入、hosted tools、custom tools、MCP、conversation、prompt template；
- 高并发、限流、取消、断线、HTTP/2、重试、长期 state 保留、其他地域或其他账号；
- OpenBridge 实际 adapter/renderer 与 Hermes Agent loop；
- strict schema 的系统性 keyword enforcement；
- 上游后续版本是否继续保持相同行为。

测试期间，一次文件搜索工具曾将 `bailian-primary` API key 回显到本地会话记录。本文不包含该值；该 key 应视为已暴露并轮换。测试没有修改仓库源码、Provider 配置或远端服务。
