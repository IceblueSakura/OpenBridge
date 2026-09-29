# 2026-09-29 Flash Provider adapter 真实验收

## 记录类型与基线

**显式 adapter 重构后的外部验收记录，含一次模型输出不符合请求格式的观察。** 测试基线固定为 [`2e18ba4cb71155941c01ff0120efc7da6587ab25`](https://github.com/IceblueSakura/OpenBridge/tree/2e18ba4cb71155941c01ff0120efc7da6587ab25)，合同见 [ADR 0008](../../architecture-v2/decisions/0008-stable-core-and-vendor-adapters.md)。本记录与此前的 [MiMo Pro 矩阵](2026-09-28-deepseek-xiaomi-provider-live-matrix.md)是不同模型、不同实现版本的证据。

使用仓库外的一次性 Rust 探测程序组合当前 `Adapter`、`admit`、`prepare`、`Attempt`、`ResponseDelivery` 与 caller-owned HTTP；下游由 OpenBridge client adapter 消费，不是官方 SDK 或生产 gateway listener。MiMo Flash 采用测试专用绑定，**不表示正式 catalog 已注册该模型**，也不把 Pro 的模型能力继承给 Flash。

## 目标、来源与执行边界

| Provider | 官方 origin | 相对入口 | 精确模型 ID |
|---|---|---|---|
| DeepSeek | `https://api.deepseek.com` | `/models`、`/chat/completions`、`/responses` | `deepseek-flash` |
| Xiaomi | `https://api.xiaomimimo.com` | `/v1/models`、`/v1/chat/completions`、`/v1/responses` | `mimo-v2.6-flash` |

两个模型 ID 均经当日官方 Models 目录核对。目录可见性不证明任意能力；本文结果来自实际生成请求。入口与字段参考分别见 [DeepSeek API](../../references/providers/deepseek-api.md)、[Xiaomi API](../../references/providers/xiaomi-api.md)，尤其不能将参考页中其他 MiMo 版本的 structured-output 声明直接扩张到 Flash。

- 每 Provider 使用单一测试凭据，经出站代理访问上述固定目标；账号身份、凭据、地域和代理地址不公开，未验证其他账号/地区。
- 只发送 synthetic 文本、工具 Schema 和固定工具结果；工具由测试程序本地提供结果，没有任意工具执行或业务数据。
- 串行请求，单次超时 120 秒、间隔 2 秒；禁止隐式重试和 HTTP 重定向。每份 body capture 上限 1 MiB，认证 headers 不进入报告。
- Rust 探测与独立 Python wire audit 分别验证：请求参数、协议闭合、IR/下游消费和输出场景正确性。**HTTP/codec 成功不等于输出满足 JSON 或工具场景。**

## 场景与独立判据

### 基础矩阵

每模型 × {Chat, Responses} × {JSON, SSE} × {文本, JSON Object, function tool}，工具场景再回传一次结果：24 个首轮场景，加 8 个工具续轮，共 32 个轮次。

| 场景 | Synthetic 输入 / 控制 | 判据 |
|---|---|---|
| 文本 | `Reply with exactly the word pong.`；输出上限 64 | 成功终态，最终正文为 `pong` |
| JSON Object | `Return a JSON object with keys pong (boolean) and note (string).`；输出上限 192；Chat `response_format.type=json_object` / Responses `text.format.type=json_object` | 独立 JSON 解析成功，`pong=true`，`note` 为字符串 |
| 单工具首轮 | 请求 `lookup` 查询 `alpha`；参数 Schema 为只含必填字符串 `key` 的 object，禁止额外属性，`strict:false`、`tool_choice:auto`；输出上限 192 | 正确 function name、call identity 和参数 `{"key":"alpha"}`，成功工具终态 |
| 单工具续轮 | 按原 call ID 回传 synthetic `{"value":42}`；输出上限 192 | 最终回答使用结果 42，不再调用工具，成功文本终态 |

### JSON 对照复测

MiMo 使用相同 JSON Object prompt 和格式参数；温度未指定与显式 `0` 分开记录。表中是**请求值**，不是从响应推断后端已完整执行温度控制。

| 协议 / 交付 | 输出上限 | temperature | 次数 | 场景通过 |
|---|---:|---|---:|---:|
| Chat / SSE | 192 | 未指定 | 3 | 3 |
| Chat / SSE | 1024 | 未指定 | 3 | 3 |
| Chat / SSE | 1024 | 0 | 3 | 3 |
| Chat / JSON | 1024 | 0 | 2 | 2 |
| Responses / SSE | 1024 | 0 | 2 | 2 |

13 次均 HTTP 200 并通过独立场景检查。前三次的请求 JSON 与下述失败样本完全一致。

### 双工具与完整历史回放

两模型 × 双协议 × 双交付，共 8 条路径、16 次生成请求；每次输出上限 1024、`temperature:0`、`parallel_tool_calls:true`，function 仍为 `lookup`。

1. 请求在同一响应中分别查询 `alpha`、`beta`；首轮必须产生两个不同 call ID，参数各自正确。
2. 回放实际 client adapter 编码的 assistant/output 历史，而非只手工重建一个 function call。
3. 按原 call ID 分别回传 synthetic 结果 42、7；续轮要求仅回答二者之和 `49`，且不再调用工具。

16 次均 HTTP 200 并通过。独立 audit 校验了工具身份与结果关联，以及上游/下游文本和 tool calls 一致。DeepSeek Responses 的两种交付路径实际回放了 reasoning item 和 encrypted replay；MiMo Responses 回放了 reasoning item，没有 encrypted replay。这不证明跨 Provider/account replay 安全或兼容。

## 结果汇总

| 模型 | 已捕获响应 | Codec / IR / 下游消费通过 | 独立场景通过 | 场景失败 |
|---|---:|---:|---:|---:|
| deepseek-flash | 24 | 24 | 24 | 0 |
| mimo-v2.6-flash | 37 | 37 | 36 | 1 |
| 合计 | 61 | 61 | 60 | 1 |

基础矩阵包含一次中断：MiMo Chat/JSON 工具第二轮只留下请求发送意图，没有响应。该 attempt 的结果和收费仍未知，不能算成功或 Provider 故障。同一 synthetic 请求另做一次补测，HTTP 200、正确返回结果 42，补齐了该场景的验收。

因此共 62 个请求/send-intent 记录，包含 61 份响应和 1 个旧中断未知 attempt。声明输出上限分别为基础矩阵 5120、独立补测 192、扩展批次 27200，累计 32512 tokens；**不是实际输出量或账单**。

中断前保存的 21 份响应 body 通过同版本库离线回放与独立 audit 恢复验收，但原 HTTP 精确状态、网络耗时未保留，未用回放参数伪造这些元数据。恢复后仅执行尚未发送的场景，并逐请求保存结果；补测与后续对照是独立记录，不覆盖原异常。

## 已观察异常：MiMo JSON Object / SSE 尾随字符

失败样本的完整请求 envelope 不入库；关键参数就是基础矩阵中的 MiMo Chat JSON Object/SSE，输出上限 192、未指定温度。拼接上游 `delta.content` 得到的最小异常正文片段为：

```text
{"pong": true, "note": "Acknowledged."}<
```

上游正常报告 `finish_reason:stop` 和 `[DONE]`，usage 为 41 output tokens，未达到 192 上限；因此没有 token 耗尽或缺少 SSE 终态的证据。独立 JSON 解析报 `Extra data: line 1 column 40 (char 39)`。尾随 `<` 来自上游，不是 OpenBridge 编码或拼接时添加的。

通用文本 codec 忠实保留正文，故 codec/IR 消费通过而 JSON Object 场景失败；不能通过裁尾把失败改为成功。后续 13 次 JSON 对照均通过，其中原请求本身也连续通过 3 次。**本轮未稳定复现该异常，不能宣称增大 token 上限或温度 0 修复了问题，也不能据一个样本认定 Flash 普遍不支持 JSON Object。**

## Usage、保真与证据保存

- 已知响应的 reported usage 合计：DeepSeek input 6429 / output 1151；MiMo input 2997 / output 1460 tokens。未知 attempt 不纳入，没有价格或账单证据，不推算费用或真实缓存收益。
- 独立 audit 确认模型 ID、输出上限、温度、JSON format 和工具参数没有在请求准备链中丢失；扩展批次逐一比对上游与下游文本、tool calls 与续轮结果关联。
- DeepSeek 有效 usage 中未报告的 cache-write detail 按已声明 adapter 规则归一为 0，保留兼容来源；MiMo 保持未报告。兼容零不是实测缓存写入或计费事实。
- 完整 synthetic captures、逐请求记录、一次性程序与独立 audit 保留在本地 ignored 产物中，不作为 checkout 测试依赖。此页仅收编脱敏场景、结果和最小异常片段，不保存凭据、账号标识、Provider request ID、reasoning 正文或完整响应。
- 临时程序的离线反例检查覆盖预算封顶、日志脱敏、错误 JSON 不被判成功、双工具参数/身份以及求和；真实结果另经独立 Python 标准库解析复核。重复网络请求具有非确定性，场景可重建不代表异常可稳定复现。

## 不证明什么

本次仅验收上述固定文本场景，不是全部模型能力清单。未执行官方 SDK gate、跨协议组合、多模态、hosted tools、负载/长期连接、实际下游背压或生产 retry/fallback；没有生产入口接线，也不是性能基准。模型绑定、任务语义和 Provider 表示合同没有因这些样本自动扩张。
