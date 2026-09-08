# Provider 探测指南

本页说明管理员显式探测的使用方式、结果解释与成本边界。基础安装与私有配置见[使用手册](../../README.md)，测试分层见[开发指南](../development.md)。

真实请求可能计费；先确认 Provider、精确请求矩阵、输出上限和脱敏报告范围。本文的命令示例不是执行授权。

## 使用方式

`openbridge-probe` 不启动下游网关，也不修改注册表。它按管理员选择的 Provider 从已经注册且已启用的 Generation Target 取得 trusted
origin、Provider path/body hook、timeout 和 credential binding；只有多个 trusted deployment 需要 `--target` 显式消歧。随后工具执行
带认证的 Models 或固定合成请求并输出脱敏 JSON：

```powershell
cargo run --locked --bin openbridge-probe -- models --provider openai
cargo run --locked --bin openbridge-probe -- generation --provider bailian --model candidate-model-id --protocol chat --delivery non-streaming --case tool-parallel-true
```

## Models 查询与 Generation 选择

`models` 输出 Provider 固定 Models endpoint 的完整计数、最多 1024 项有界 ID 样本，以及可选 `--model` 的可见性。
`generation` 需要 `--model`，一次只执行一个由 `--protocol`（chat/responses）、`--delivery`（non-streaming/streaming）和
`--case` 选择的请求；默认是 Chat、non-streaming 与 text。case 为
text/reasoning-none/reasoning-minimal/reasoning-low/reasoning-medium/reasoning-high/reasoning-xhigh/reasoning-max/json-object/
json-schema/json-schema-strict/image-input-inline-png/tool-auto/tool-none/tool-required/tool-named/tool-strict/tool-parallel-false/
tool-parallel-true/reasoning-summary/include-encrypted-content/prompt-cache-key。
其中后三个为 Responses-only 单字段差分 case。
CLI 和 library 不接受 `all`、列表或内置笛卡尔矩阵；外部测试脚本通过多次独立调用编排。

## 固定 case 与结果判定

Structured case 携带固定冲突 prompt 与固定 `{"probe":"ok"}` schema；tool case 携带两个以内固定 function tools、固定 prompt 与
固定 arguments schema，只观察单次首轮响应中的 tool choice、strict 和 parallel 差分，不执行工具、不发送 tool result，也不发起
continuation。两类 case 都按完整 terminal 与瞬时输出给出 `supported`、`not_honored` 或 `inconclusive`，不保留生成文本、tool
arguments、call ID 或 item ID。`auto` 未调用工具和 `parallel=true` 只返回一个调用都记为 `inconclusive`，不误报不支持。

## 自定义输入

管理员可以为非 tool case 用 `--prompt <text>`（≤ 4 KiB）替换该 case 的固定用户 prompt，并可以为 `json-schema` /
`json-schema-strict` case 用 `--schema <json>`（≤ 8 KiB 的 JSON object）与 `--schema-name <name>` 替换响应格式对象与名称；
`--prompt` 对 tool case 拒绝，`--schema`/`--schema-name` 对其他 case 拒绝。带自定义 `--schema` 的 case 因无固定 oracle 而
恒为 `inconclusive` verdict，schema 接受性由 `accepted`/`rejected` outcome 体现；报告为每个生效覆盖记录
`custom_prompt_fingerprint`、`custom_schema_fingerprint`（各自内容的 SHA-256 前 16 位十六进制）与 `custom_schema_name`，
evidence 归属由外部脚本记录指纹与原文的对应，报告本体从不包含覆盖文本。无覆盖时固定 case 的 wire、oracle 与 verdict 保持
canonical 不变。

## 图片与 Responses 差分

`image-input-inline-png` 使用内置、已视觉复核的固定 PNG data URL；Chat 发送 `image_url`，Responses 发送 `input_image`。只有完整响应
精确返回图片中的固定 token 才记为 `supported`，请求成功但识别不匹配记为 `inconclusive`，报告不保留图片 data URL、prompt 或输出正文。
`reasoning-summary` 发送固定 `reasoning: {"effort":"medium","summary":"auto"}` 并只观察响应是否出现非空 reasoning summary（`reasoning_summary_observed` 布尔），不保留 summary 文本；`include-encrypted-content` 发送固定 `include: ["reasoning.encrypted_content"]` 并观察接受性，不验证加密内容本身；`prompt-cache-key` 发送固定 `prompt_cache_key` hint 并观察接受性，不承诺也不验证缓存效果。三者均为单字段差分：除被探测字段外其余 wire 形状保持与对应 baseline 一致，接受性由 `accepted`/`rejected` outcome 体现。

## 目标、输出上限与成本

`--target` 仅在多个 trusted deployment 之间显式消歧；Provider 解析只接受已注册且启用的 Generation Target。所有 bounded
Generation case 使用固定 4096-token accuracy-oriented upstream output limit；探测 Target 自身已注册 upstream model 时按其 output ceiling
下调，显式 candidate model 不继承另一模型的 ceiling。只有 backend 明确
拒绝该字段时才使用 `--allow-unbounded-streaming-output` 放开 streaming limit，这可能增加 reasoning 时间和计费。

`--model` 允许在正式注册前把同一个 candidate model ID 用于 `models` 可见性与 `generation` case；所选 Provider 解析只接受
Generation task Target，Embeddings/Images/Audio Target 不能借 Provider-wide path 发送 Generation。该参数不能覆盖 endpoint、
relative path、credential、认证 header 或任意 JSON 结构；`--prompt` 与 `--schema`/`--schema-name` 只替换上述固定合成请求的
用户 prompt 文本与响应格式对象，不改变 operation、工具定义或图片负载。

## 报告与验收边界

每个 case 独立报告 `accepted`、`rejected`、`unsupported` 或 `inconclusive`、HTTP status、耗时、标准 token usage、失败阶段及有界协议元数据；报告不包含
credential、认证 header、完整请求正文、生成正文或完整 upstream response body。
`unsupported` 只表示本地 trusted Target/profile 不允许 operation 或 delivery；真实 upstream 的所有非 2xx（包括 candidate-model 404）
都只是该请求的 `rejected`，不会提升为 endpoint 静态结论。

一次 `accepted` 或 capability oracle 的 `supported` 只证明该固定首轮请求当时取得相应 JSON/SSE 结果；它不证明 reasoning 参数实际生效、
完整工具调用流程、工具执行/续轮、能力稳定，或 inline PNG 之外的 remote/detail/其他多模态能力，也不证明模型质量、SDK/Agent 兼容、
retry/fallback、负载或长期稳定性。完整说明见
[当前状态边界](../implementation-status/current-boundaries.md)。
